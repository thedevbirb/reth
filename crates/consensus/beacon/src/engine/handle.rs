//! `BeaconConsensusEngine` external API

use crate::{
    engine::message::OnForkChoiceUpdated, BeaconConsensusEngineEvent, BeaconEngineMessage,
    BeaconForkChoiceUpdateError, BeaconOnNewPayloadError,
};
use futures::TryFutureExt;
use reth_interfaces::RethResult;
use reth_node_api::EngineTypes;
use reth_rpc_types::engine::{
    CancunPayloadFields, ExecutionPayload, ForkchoiceState, ForkchoiceUpdated, PayloadStatus,
};
use tokio::sync::{mpsc, mpsc::UnboundedSender, oneshot};
use tokio_stream::wrappers::UnboundedReceiverStream;

/// A _shareable_ beacon consensus frontend type. Used to interact with the spawned beacon consensus
/// engine task.
///
/// See also [`BeaconConsensusEngine`](crate::engine::BeaconConsensusEngine).
#[derive(Debug)]
pub struct BeaconConsensusEngineHandle<Engine>
where
    Engine: EngineTypes,
{
    pub(crate) to_engine: UnboundedSender<BeaconEngineMessage<Engine>>,
}

impl<Engine> Clone for BeaconConsensusEngineHandle<Engine>
where
    Engine: EngineTypes,
{
    fn clone(&self) -> Self {
        Self { to_engine: self.to_engine.clone() }
    }
}

// === impl BeaconConsensusEngineHandle ===

impl<Engine> BeaconConsensusEngineHandle<Engine>
where
    Engine: EngineTypes,
{
    /// Creates a new beacon consensus engine handle.
    pub fn new(to_engine: UnboundedSender<BeaconEngineMessage<Engine>>) -> Self {
        Self { to_engine }
    }

    /// Sends a new payload message to the beacon consensus engine and waits for a response.
    ///
    /// See also <https://github.com/ethereum/execution-apis/blob/3d627c95a4d3510a8187dd02e0250ecb4331d27e/src/engine/shanghai.md#engine_newpayloadv2>
    pub async fn new_payload(
        &self,
        payload: ExecutionPayload,
        cancun_fields: Option<CancunPayloadFields>,
    ) -> Result<PayloadStatus, BeaconOnNewPayloadError> {
        let (tx, rx) = oneshot::channel();
        let _ = self.to_engine.send(BeaconEngineMessage::NewPayload { payload, cancun_fields, tx });
        rx.await.map_err(|_| BeaconOnNewPayloadError::EngineUnavailable)?
    }

    /// Sends a forkchoice update message to the beacon consensus engine and waits for a response.
    ///
    /// See also <https://github.com/ethereum/execution-apis/blob/3d627c95a4d3510a8187dd02e0250ecb4331d27e/src/engine/shanghai.md#engine_forkchoiceupdatedv2>
    pub async fn fork_choice_updated(
        &self,
        state: ForkchoiceState,
        payload_attrs: Option<Engine::PayloadAttributes>,
    ) -> Result<ForkchoiceUpdated, BeaconForkChoiceUpdateError> {
        Ok(self
            .send_fork_choice_updated(state, payload_attrs)
            .map_err(|_| BeaconForkChoiceUpdateError::EngineUnavailable)
            .await??
            .await?)
    }

    /// Sends a forkchoice update message to the beacon consensus engine and returns the receiver to
    /// wait for a response.
    fn send_fork_choice_updated(
        &self,
        state: ForkchoiceState,
        payload_attrs: Option<Engine::PayloadAttributes>,
    ) -> oneshot::Receiver<RethResult<OnForkChoiceUpdated>> {
        let (tx, rx) = oneshot::channel();
        let _ = self.to_engine.send(BeaconEngineMessage::ForkchoiceUpdated {
            state,
            payload_attrs,
            tx,
        });
        rx
    }

    /// Sends a transition configuration exchange message to the beacon consensus engine.
    ///
    /// See also <https://github.com/ethereum/execution-apis/blob/3d627c95a4d3510a8187dd02e0250ecb4331d27e/src/engine/paris.md#engine_exchangetransitionconfigurationv1>
    pub async fn transition_configuration_exchanged(&self) {
        let _ = self.to_engine.send(BeaconEngineMessage::TransitionConfigurationExchanged);
    }

    /// Creates a new [`BeaconConsensusEngineEvent`] listener stream.
    pub fn event_listener(&self) -> UnboundedReceiverStream<BeaconConsensusEngineEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        let _ = self.to_engine.send(BeaconEngineMessage::EventListener(tx));
        UnboundedReceiverStream::new(rx)
    }
}
