//! Executor do grafo de agentes: nós rodam em paralelo quando as dependências terminam e podem criar
//! novos nós em tempo de execução (retrabalho, pedidos de assets), o que torna o fluxo não linear.

use super::run::RunEvent;
use super::{Ctx, roles};
use futures_util::FutureExt;
use std::collections::HashSet;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::Instant;
use tokio::task::JoinSet;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Role {
    Director,
    Writer,
    Architect,
    Artist,
    Composer,
    Engineer,
    Integrator,
    Tester,
    Publisher,
}

impl Role {
    pub fn key(self) -> &'static str {
        match self {
            Role::Director => "director",
            Role::Writer => "writer",
            Role::Architect => "architect",
            Role::Artist => "artist",
            Role::Composer => "composer",
            Role::Engineer => "engineer",
            Role::Integrator => "integrator",
            Role::Tester => "tester",
            Role::Publisher => "publisher",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Role::Director => "Diretor",
            Role::Writer => "Roteirista",
            Role::Architect => "Arquiteto",
            Role::Artist => "Artista",
            Role::Composer => "Compositor",
            Role::Engineer => "Engenheiro",
            Role::Integrator => "Integrador",
            Role::Tester => "Testador",
            Role::Publisher => "Publicador",
        }
    }

    /// Qual modelo configurado o agente usa: text | code | image | sound
    pub fn task(self) -> &'static str {
        match self {
            Role::Director | Role::Writer => "text",
            Role::Architect | Role::Engineer | Role::Integrator | Role::Tester | Role::Publisher => "code",
            Role::Artist => "image",
            Role::Composer => "sound",
        }
    }

    /// Falha de agente "suave" não impede os dependentes de rodar (há fallback embutido).
    pub fn soft(self) -> bool {
        matches!(self, Role::Artist | Role::Composer | Role::Writer)
    }
}

#[derive(Clone, Debug)]
pub struct NodeSpec {
    pub id: String,
    pub role: Role,
    pub deps: Vec<String>,
    pub round: u32,
}

#[derive(Default)]
pub struct Outcome {
    pub spawn: Vec<NodeSpec>,
    /// (id antigo, novos ids): reaponta dependências de nós ainda pendentes.
    pub redirect: Vec<(String, Vec<String>)>,
    pub detail: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Status {
    Pending,
    Running,
    Done,
    Failed,
    Skipped,
}

struct Node {
    spec: NodeSpec,
    status: Status,
}

fn emit_added(ctx: &Ctx, spec: &NodeSpec) {
    ctx.run.emit(RunEvent::NodeAdded { id: spec.id.clone(), role: spec.role.key().into(), label: spec.role.label().into(), task: spec.role.task().into(), deps: spec.deps.clone(), round: spec.round });
}

fn initial(ctx: &Ctx) -> Vec<NodeSpec> {
    let mk = |role: Role, deps: &[&str]| NodeSpec { id: ctx.next_id(role.key()), role, deps: deps.iter().map(|s| s.to_string()).collect(), round: 1 };
    vec![
        mk(Role::Director, &[]),
        mk(Role::Writer, &["director"]),
        mk(Role::Architect, &["director"]),
        mk(Role::Artist, &["director"]),
        mk(Role::Composer, &["director"]),
        mk(Role::Engineer, &["writer", "architect"]),
        mk(Role::Integrator, &["engineer", "artist", "composer"]),
        mk(Role::Tester, &["integrator"]),
        mk(Role::Publisher, &["tester"]),
    ]
}

/// Aplica o resultado de um nó: 1º reaponta dependências dos nós que JÁ existiam (ex.: publicador passa a esperar
/// o novo testador) e só depois acrescenta os nós novos — que dependem do nó atual e não podem ser reapontados
/// (senão formariam um ciclo). Devolve as specs a (re)emitir para a interface.
fn apply_outcome(nodes: &mut Vec<Node>, redirect: Vec<(String, Vec<String>)>, spawn: Vec<NodeSpec>) -> Vec<NodeSpec> {
    let mut emit = vec![];
    for (old, new) in redirect {
        for n in nodes.iter_mut().filter(|n| n.status == Status::Pending) {
            if n.spec.deps.contains(&old) {
                n.spec.deps.retain(|d| d != &old);
                n.spec.deps.extend(new.iter().cloned());
                emit.push(n.spec.clone());
            }
        }
    }
    let known: HashSet<String> = nodes.iter().map(|n| n.spec.id.clone()).collect();
    for s in spawn {
        if known.contains(&s.id) {
            continue;
        }
        emit.push(s.clone());
        nodes.push(Node { spec: s, status: Status::Pending });
    }
    emit
}

/// Executa o pipeline. Retorna `true` se o Publicador concluiu.
pub async fn execute(ctx: Arc<Ctx>) -> bool {
    let mut nodes: Vec<Node> = vec![];
    for s in initial(&ctx) {
        emit_added(&ctx, &s);
        nodes.push(Node { spec: s, status: Status::Pending });
    }
    let mut set: JoinSet<(String, std::thread::Result<anyhow::Result<Outcome>>, f32)> = JoinSet::new();

    loop {
        if ctx.run.cancelled() {
            break;
        }
        // 1) propaga "pulado" e descobre quem está pronto
        let mut ready: Vec<usize> = vec![];
        loop {
            let mut changed = false;
            ready.clear();
            for i in 0..nodes.len() {
                if nodes[i].status != Status::Pending {
                    continue;
                }
                let (mut blocked, mut skip) = (false, false);
                for d in &nodes[i].spec.deps {
                    match nodes.iter().find(|n| &n.spec.id == d) {
                        Some(n) => match n.status {
                            Status::Done => {}
                            Status::Failed if n.spec.role.soft() => {}
                            Status::Failed | Status::Skipped => skip = true,
                            Status::Pending | Status::Running => blocked = true,
                        },
                        None => skip = true,
                    }
                }
                if skip {
                    nodes[i].status = Status::Skipped;
                    ctx.run.emit(RunEvent::NodeStatus { id: nodes[i].spec.id.clone(), status: "skipped".into(), detail: Some("dependência falhou".into()) });
                    changed = true;
                } else if !blocked {
                    ready.push(i);
                }
            }
            if !changed {
                break;
            }
        }
        // 2) dispara
        for &i in &ready {
            nodes[i].status = Status::Running;
            let spec = nodes[i].spec.clone();
            ctx.run.emit(RunEvent::NodeStatus { id: spec.id.clone(), status: "running".into(), detail: None });
            let c = ctx.clone();
            set.spawn(async move {
                let t0 = Instant::now();
                let id = spec.id.clone();
                let res = AssertUnwindSafe(roles::run(c, spec)).catch_unwind().await;
                (id, res, t0.elapsed().as_secs_f32())
            });
        }
        if set.is_empty() {
            break;
        }
        // 3) espera alguém terminar
        let Some(joined) = set.join_next().await else { break };
        let Ok((id, res, secs)) = joined else { continue };
        let Some(idx) = nodes.iter().position(|n| n.spec.id == id) else { continue };
        match res {
            Ok(Ok(outcome)) => {
                nodes[idx].status = Status::Done;
                let detail = match outcome.detail {
                    Some(d) => format!("{d} · {secs:.1}s"),
                    None => format!("{secs:.1}s"),
                };
                ctx.run.emit(RunEvent::NodeStatus { id: id.clone(), status: "done".into(), detail: Some(detail) });
                for n in apply_outcome(&mut nodes, outcome.redirect, outcome.spawn) {
                    emit_added(&ctx, &n);
                }
            }
            Ok(Err(e)) => {
                nodes[idx].status = Status::Failed;
                ctx.log(&id, "error", format!("{e:#}"));
                ctx.run.emit(RunEvent::NodeStatus { id: id.clone(), status: "failed".into(), detail: Some(format!("{e:#}")) });
            }
            Err(_) => {
                nodes[idx].status = Status::Failed;
                ctx.log(&id, "error", "erro interno inesperado no agente");
                ctx.run.emit(RunEvent::NodeStatus { id: id.clone(), status: "failed".into(), detail: Some("erro interno".into()) });
            }
        }
    }
    if ctx.run.cancelled() {
        set.abort_all();
        for n in nodes.iter_mut().filter(|n| matches!(n.status, Status::Pending | Status::Running)) {
            n.status = Status::Skipped;
            ctx.run.emit(RunEvent::NodeStatus { id: n.spec.id.clone(), status: "skipped".into(), detail: Some("cancelado".into()) });
        }
        return false;
    }
    nodes.iter().any(|n| n.spec.role == Role::Publisher && n.status == Status::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(id: &str, role: Role, deps: &[&str], round: u32) -> NodeSpec {
        NodeSpec { id: id.into(), role, deps: deps.iter().map(|d| d.to_string()).collect(), round }
    }

    #[test]
    fn rework_chain_does_not_create_a_cycle() {
        let mut nodes =
            vec![Node { spec: spec("tester", Role::Tester, &["integrator"], 1), status: Status::Done }, Node { spec: spec("publisher", Role::Publisher, &["tester"], 1), status: Status::Pending }];
        // o testador pede retrabalho: engineer#2 -> integrator#2 -> tester#2, e o publicador passa a esperar tester#2
        let spawn = vec![spec("engineer#2", Role::Engineer, &["tester"], 2), spec("integrator#2", Role::Integrator, &["engineer#2"], 2), spec("tester#2", Role::Tester, &["integrator#2"], 2)];
        apply_outcome(&mut nodes, vec![("tester".into(), vec!["tester#2".into()])], spawn);
        let deps = |id: &str| nodes.iter().find(|n| n.spec.id == id).unwrap().spec.deps.clone();
        assert_eq!(deps("publisher"), vec!["tester#2"]);
        assert_eq!(deps("engineer#2"), vec!["tester"], "o novo engenheiro deve continuar dependendo do testador original");
        // sem ciclos: seguindo as dependências a partir do publicador sempre se chega a um nó sem dependências pendentes
        fn reaches(nodes: &[Node], from: &str, target: &str, depth: usize) -> bool {
            if depth > 20 {
                return true;
            } // ciclo
            nodes.iter().find(|n| n.spec.id == from).map(|n| n.spec.deps.iter().any(|d| d == target || reaches(nodes, d, target, depth + 1))).unwrap_or(false)
        }
        assert!(!reaches(&nodes, "publisher", "publisher", 0));
    }

    #[test]
    fn spawned_ids_are_not_duplicated() {
        let mut nodes = vec![Node { spec: spec("a", Role::Artist, &[], 1), status: Status::Done }];
        let e = apply_outcome(&mut nodes, vec![], vec![spec("a", Role::Artist, &[], 1), spec("b", Role::Artist, &["a"], 2)]);
        assert_eq!(nodes.len(), 2);
        assert_eq!(e.len(), 1);
    }
}
