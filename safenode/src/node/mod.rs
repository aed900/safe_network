// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod api;
mod error;
mod event;

pub use self::event::NodeEvent;

use self::{
    error::{Error, Result},
    event::NodeEventsChannel,
};

use crate::{
    network::Network,
    protocol::{
        messages::{
            Cmd, CmdResponse, Query, QueryResponse, RegisterCmd, RegisterQuery, Request, Response,
            SignedRegisterCreate, SignedRegisterEdit,
        },
        types::{
            address::{ChunkAddress, RegisterAddress},
            chunk::Chunk,
            error::Error as ProtocolError,
            register::Register,
        },
    },
    storage::DataStorage,
};

/// `Node` represents a single node in the distributed network. It handles
/// network events, processes incoming requests, interacts with the data
/// storage, and broadcasts node-related events.
#[derive(Clone)]
pub struct Node {
    network: Network,
    storage: DataStorage,
    events_channel: NodeEventsChannel,
}

// --------------------------------------------------------------------------------------------------------
// ---------------------------------  Client implementation -----------------------------------------------
// --------------------------------------------------------------------------------------------------------

/// A client to store and get data.
pub struct Client {
    node: Node,
}

impl Client {
    /// A new client.
    pub fn new(node: Node) -> Self {
        Self { node }
    }

    /// Store `Chunk` to its close group.
    pub async fn store_chunk(&self, chunk: Chunk) -> Result<()> {
        info!("Store chunk: {:?}", chunk.address());
        let request = Request::Cmd(Cmd::StoreChunk(chunk));
        let responses = self.send_to_closest(request).await?;

        let all_ok = responses
            .iter()
            .all(|resp| matches!(resp, Ok(Response::Cmd(CmdResponse::StoreChunk(Ok(()))))));
        if all_ok {
            return Ok(());
        }

        // If not all were Ok, we will return the first error sent to us.
        for resp in responses.iter().flatten() {
            if let Response::Cmd(CmdResponse::StoreChunk(result)) = resp {
                result.clone()?;
            };
        }

        // If there were no success or fail to the expected query,
        // we check if there were any send errors.
        for resp in responses {
            let _ = resp?;
        }

        // If there were no store chunk errors, then we had unexpected responses.
        Err(Error::Protocol(ProtocolError::UnexpectedResponses))
    }

    ///
    pub async fn create_register(&self, cmd: SignedRegisterCreate) -> Result<()> {
        info!("Create register: {:?}", cmd.dst());
        let request = Request::Cmd(Cmd::Register(RegisterCmd::Create(cmd)));
        let responses = self.send_to_closest(request).await?;

        let all_ok = responses
            .iter()
            .all(|resp| matches!(resp, Ok(Response::Cmd(CmdResponse::CreateRegister(Ok(()))))));
        if all_ok {
            return Ok(());
        }

        // If not all were Ok, we will return the first error sent to us.
        for resp in responses.iter().flatten() {
            if let Response::Cmd(CmdResponse::CreateRegister(result)) = resp {
                result.clone()?;
            };
        }

        // If there were no success or fail to the expected query,
        // we check if there were any send errors.
        for resp in responses {
            let _ = resp?;
        }

        // If there were no register errors, then we had unexpected responses.
        Err(Error::Protocol(ProtocolError::UnexpectedResponses))
    }

    ///
    pub async fn edit_register(&self, cmd: SignedRegisterEdit) -> Result<()> {
        info!("Create register: {:?}", cmd.dst());
        let request = Request::Cmd(Cmd::Register(RegisterCmd::Edit(cmd)));
        let responses = self.send_to_closest(request).await?;

        let all_ok = responses
            .iter()
            .all(|resp| matches!(resp, Ok(Response::Cmd(CmdResponse::EditRegister(Ok(()))))));
        if all_ok {
            return Ok(());
        }

        // If not all were Ok, we will return the first error sent to us.
        for resp in responses.iter().flatten() {
            if let Response::Cmd(CmdResponse::EditRegister(result)) = resp {
                result.clone()?;
            };
        }

        // If there were no success or fail to the expected query,
        // we check if there were any send errors.
        for resp in responses {
            let _ = resp?;
        }

        // If there were no register errors, then we had unexpected responses.
        Err(Error::Protocol(ProtocolError::UnexpectedResponses))
    }

    /// Retrieve a `Chunk` from the closest peers
    pub async fn get_chunk(&self, address: ChunkAddress) -> Result<Chunk> {
        info!("Get chunk: {address:?}");
        let request = Request::Query(Query::GetChunk(address));
        let responses = self.send_to_closest(request).await?;

        // We will return the first chunk we get.
        for resp in responses.iter().flatten() {
            if let Response::Query(QueryResponse::GetChunk(Ok(chunk))) = resp {
                return Ok(chunk.clone());
            };
        }

        // If no chunk was found, we will return the first error sent to us.
        for resp in responses.iter().flatten() {
            if let Response::Query(QueryResponse::GetChunk(result)) = resp {
                let _ = result.clone()?;
            };
        }

        // If there were no success or fail to the expected query,
        // we check if there were any send errors.
        for resp in responses {
            let _ = resp?;
        }

        // If there was none of the above, then we had unexpected responses.
        Err(Error::Protocol(ProtocolError::UnexpectedResponses))
    }

    /// Retrieve a `Register` from the closest peers
    pub async fn get_register(&self, address: RegisterAddress) -> Result<Register> {
        info!("Get chunk: {address:?}");
        let request = Request::Query(Query::Register(RegisterQuery::Get(address)));
        let responses = self.send_to_closest(request).await?;

        // We will return the first register we get.
        for resp in responses.iter().flatten() {
            if let Response::Query(QueryResponse::GetRegister(Ok(register))) = resp {
                return Ok(register.clone());
            };
        }

        // If no register was gotten, we will return the first error sent to us.
        for resp in responses.iter().flatten() {
            if let Response::Query(QueryResponse::GetChunk(result)) = resp {
                let _ = result.clone()?;
            };
        }

        // If there were no success or fail to the expected query,
        // we check if there were any send errors.
        for resp in responses {
            let _ = resp?;
        }

        // If there was none of the above, then we had unexpected responses.
        Err(Error::Protocol(ProtocolError::UnexpectedResponses))
    }

    async fn send_to_closest(&self, request: Request) -> Result<Vec<Result<Response>>> {
        info!("Sending {:?} to the closest peers", request.dst());
        let closest_peers = self
            .node
            .network
            .get_closest_peers(*request.dst().name())
            .await?;
        Ok(self
            .node
            .send_req_and_get_responses(closest_peers, &request, true)
            .await)
    }
}