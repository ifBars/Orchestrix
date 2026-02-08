use crate::runtime::orchestrator::Orchestrator;

pub async fn recover(orchestrator: &Orchestrator) {
    orchestrator.recover_active_runs().await;
}
