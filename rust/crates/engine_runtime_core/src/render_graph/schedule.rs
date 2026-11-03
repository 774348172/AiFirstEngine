//! Resource access compiler belonging to the existing RenderGraph, independent of a GPU API.
use super::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GraphResourceId(pub String);
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GraphPassId(pub String);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphAspect {
    Color,
    Depth,
    Buffer,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphAccessKind {
    Read,
    Write,
    AttachmentReadWrite,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphAccess {
    pub resource: GraphResourceId,
    pub aspect: GraphAspect,
    pub mip: u32,
    pub layer: u32,
    pub kind: GraphAccessKind,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphHistoryIdentity {
    pub device_epoch: u64,
    pub origin_epoch: u64,
    pub camera_id: u64,
    pub resource_version: u64,
    pub extent: [u32; 2],
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphResourceShape {
    pub mip_levels: u32,
    pub layers: u32,
    pub samples: u32,
    pub initialized: bool,
    pub history: Option<GraphHistoryIdentity>,
}
impl Default for GraphResourceShape {
    fn default() -> Self {
        Self {
            mip_levels: 1,
            layers: 1,
            samples: 1,
            initialized: false,
            history: None,
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphContracts {
    pub resources: BTreeMap<GraphResourceId, GraphResourceShape>,
    pub accesses: BTreeMap<GraphPassId, Vec<GraphAccess>>,
    /// Explicit dependencies are separate from inferred resource hazards.
    pub before: Vec<(GraphPassId, GraphPassId)>,
}
impl GraphContracts {
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty() && self.accesses.is_empty() && self.before.is_empty()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphResourceLifetimeRange {
    pub resource: GraphResourceId,
    pub first: usize,
    pub last: usize,
    /// Only frame-local, descriptor-compatible, non-overlapping resources alias.
    pub transient_slot: Option<u32>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledGraphSchedule {
    pub resources: Vec<RenderResource>,
    pub resource_shapes: BTreeMap<GraphResourceId, GraphResourceShape>,
    pub order: Vec<usize>,
    pub dependencies: Vec<(usize, usize)>,
    pub lifetimes: Vec<GraphResourceLifetimeRange>,
    pub history: BTreeMap<GraphResourceId, GraphHistoryIdentity>,
}
fn error(code: &str, where_: impl Into<String>) -> RenderGraphDiagnostic {
    RenderGraphDiagnostic::error(code, where_)
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct GraphLayout {
    resources: Vec<RenderResource>,
    contracts: GraphContracts,
    passes: Vec<PassLayout>,
}
type PassLayout = (
    String,
    RenderPassKind,
    Vec<String>,
    Vec<String>,
    Vec<String>,
    Option<String>,
    bool,
);
/// One last structural plan per renderer; commands and frame/view values are patched separately.
#[derive(Debug, Clone, Default)]
pub struct GraphScheduleCache {
    last: Option<(GraphLayout, Arc<CompiledGraphSchedule>)>,
    // Successful immutable draw proofs, never resource residency or graph validity.
    pub(super) geometry_draw_proofs: Vec<std::sync::Weak<crate::render_geometry::DrawGeometry>>,
    #[cfg(test)]
    pub(crate) geometry_validation_visits: u64,
    pub compilations: u64,
    pub reuses: u64,
}
impl GraphScheduleCache {
    pub fn compile(
        &mut self,
        graph: &RenderGraph,
    ) -> Result<Arc<CompiledGraphSchedule>, Vec<RenderGraphDiagnostic>> {
        let key = GraphLayout {
            resources: graph.resources.clone(),
            contracts: graph.contracts.clone(),
            passes: graph
                .passes
                .iter()
                .map(|p| {
                    (
                        p.pass_id.clone(),
                        p.pass_kind,
                        p.reads.clone(),
                        p.writes.clone(),
                        p.color_targets.clone(),
                        p.depth_target.clone(),
                        p.commands.iter().any(|c| {
                            matches!(c, RenderPassCommand::BeginView {view, ..}
                            if view.is_shadow_map() && view.depth_clear.is_none())
                        }),
                    )
                })
                .collect(),
        };
        if let Some((old, plan)) = &self.last {
            if *old == key {
                self.reuses += 1;
                return Ok(plan.clone());
            }
        }
        let plan = Arc::new(graph.compile_schedule()?);
        self.compilations += 1;
        self.last = Some((key, plan.clone()));
        Ok(plan)
    }
}
fn topo(n: usize, edges: &BTreeSet<(usize, usize)>) -> Option<Vec<usize>> {
    let mut degree = vec![0; n];
    let mut outgoing = vec![Vec::new(); n];
    for &(a, b) in edges {
        degree[b] += 1;
        outgoing[a].push(b);
    }
    let mut ready = (0..n).filter(|&i| degree[i] == 0).collect::<BTreeSet<_>>();
    let mut out = Vec::new();
    while let Some(i) = ready.pop_first() {
        out.push(i);
        for &b in &outgoing[i] {
            degree[b] -= 1;
            if degree[b] == 0 {
                ready.insert(b);
            }
        }
    }
    (out.len() == n).then_some(out)
}
impl RenderGraph {
    pub fn compile_schedule(&self) -> Result<CompiledGraphSchedule, Vec<RenderGraphDiagnostic>> {
        let mut errors = Vec::new();
        let mut resources = BTreeMap::new();
        let mut pass_ids = BTreeMap::new();
        for r in &self.resources {
            if resources.insert(r.resource_id.clone(), r).is_some() {
                errors.push(error("graph.duplicate_resource", &r.resource_id));
            }
            if r.size.width == 0
                || r.size.height == 0
                || r.format == RenderResourceFormat::Unknown
                    && r.resource_kind != RenderResourceKind::Buffer
            {
                errors.push(error("graph.invalid_descriptor", &r.resource_id));
            }
        }
        for (i, p) in self.passes.iter().enumerate() {
            if pass_ids.insert(p.pass_id.clone(), i).is_some() {
                errors.push(error("graph.duplicate_pass", &p.pass_id));
            }
        }
        for id in self.contracts.resources.keys() {
            if !resources.contains_key(&id.0) {
                errors.push(error("graph.missing_resource", &id.0));
            }
        }
        for id in self.contracts.accesses.keys() {
            if !pass_ids.contains_key(&id.0) {
                errors.push(error("graph.missing_pass", &id.0));
            }
        }
        let mut edges = BTreeSet::new();
        for (a, b) in &self.contracts.before {
            match (pass_ids.get(&a.0), pass_ids.get(&b.0)) {
                (Some(&a), Some(&b)) => {
                    edges.insert((a, b));
                }
                _ => errors.push(error(
                    "graph.missing_dependency",
                    format!("{} -> {}", a.0, b.0),
                )),
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let Some(base) = topo(self.passes.len(), &edges) else {
            return Err(vec![error("graph.cycle", &self.graph_id)]);
        };
        let mut last_write = BTreeMap::<(String, u32, u32), usize>::new();
        let mut readers = BTreeMap::<(String, u32, u32), BTreeSet<usize>>::new();
        let mut initialized = BTreeSet::new();
        let mut uses = BTreeMap::<String, Vec<usize>>::new();
        let mut history = BTreeMap::new();
        for (id, r) in &resources {
            let shape = self
                .contracts
                .resources
                .get(&GraphResourceId(id.clone()))
                .cloned()
                .unwrap_or_default();
            if shape.mip_levels == 0
                || shape.mip_levels > 32 - r.size.width.max(r.size.height).leading_zeros()
                || shape.layers == 0
                || shape.layers > 2048
                || !matches!(shape.samples, 1 | 2 | 4 | 8 | 16)
                || shape.samples > 1 && shape.mip_levels > 1
            {
                errors.push(error("graph.invalid_shape", id));
                continue;
            }
            if let Some(h) = &shape.history {
                if h.extent != [r.size.width, r.size.height]
                    || r.lifetime != RenderResourceLifetime::Persistent
                {
                    errors.push(error("graph.invalid_history", id));
                }
                history.insert(GraphResourceId(id.clone()), h.clone());
            }
            if shape.initialized || r.lifetime == RenderResourceLifetime::External {
                for m in 0..shape.mip_levels {
                    for l in 0..shape.layers {
                        initialized.insert((id.clone(), m, l));
                    }
                }
            }
        }
        for &i in &base {
            let p = &self.passes[i];
            let mut access = Vec::new();
            let aspect = |id: &str| {
                if resources
                    .get(id)
                    .is_some_and(|r| r.resource_kind == RenderResourceKind::Buffer)
                {
                    GraphAspect::Buffer
                } else if resources.get(id).is_some_and(|r| {
                    matches!(
                        r.format,
                        RenderResourceFormat::Depth32Float | RenderResourceFormat::Depth24Plus
                    )
                }) {
                    GraphAspect::Depth
                } else {
                    GraphAspect::Color
                }
            };
            // Existing pass declarations remain the default producer; overrides cannot omit declared resources.
            for id in &p.reads {
                access.push(GraphAccess {
                    resource: GraphResourceId(id.clone()),
                    aspect: aspect(id),
                    mip: 0,
                    layer: 0,
                    kind: GraphAccessKind::Read,
                });
            }
            for id in p
                .writes
                .iter()
                .chain(p.color_targets.iter())
                .chain(p.depth_target.iter())
            {
                if access
                    .iter()
                    .any(|a| a.resource.0 == *id && a.kind != GraphAccessKind::Read)
                {
                    continue;
                }
                let initializes = matches!(
                    p.pass_kind,
                    RenderPassKind::ResolveColor3d
                        | RenderPassKind::Clear
                        | RenderPassKind::BeginView
                ) && !p.commands.iter().any(|c| {
                    matches!(c,
                        RenderPassCommand::BeginView { view, depth_target: Some(depth), .. }
                        if depth == id && view.is_shadow_map()
                            && view.depth_clear.is_none())
                });
                access.push(GraphAccess {
                    resource: GraphResourceId(id.clone()),
                    aspect: aspect(id),
                    mip: 0,
                    layer: 0,
                    kind: if initializes {
                        GraphAccessKind::Write
                    } else {
                        GraphAccessKind::AttachmentReadWrite
                    },
                });
            }
            if let Some(explicit) = self.contracts.accesses.get(&GraphPassId(p.pass_id.clone())) {
                for a in &access {
                    if !explicit.iter().any(|e| {
                        e.resource == a.resource
                            && e.aspect == a.aspect
                            && e.mip == 0
                            && e.layer == 0
                            && match a.kind {
                                GraphAccessKind::Read => e.kind != GraphAccessKind::Write,
                                GraphAccessKind::Write => e.kind != GraphAccessKind::Read,
                                GraphAccessKind::AttachmentReadWrite => {
                                    e.kind == GraphAccessKind::AttachmentReadWrite
                                }
                            }
                    }) {
                        errors.push(error(
                            "graph.undeclared_access",
                            format!("{}:{}", p.pass_id, a.resource.0),
                        ));
                    }
                }
                access = explicit.clone();
            }
            let attachments = p
                .color_targets
                .iter()
                .chain(p.depth_target.iter())
                .filter_map(|id| resources.get(id).copied())
                .collect::<Vec<_>>();
            if let Some(first) = attachments.first() {
                let first_shape = self
                    .contracts
                    .resources
                    .get(&GraphResourceId(first.resource_id.clone()))
                    .cloned()
                    .unwrap_or_default();
                for r in &attachments {
                    let s = self
                        .contracts
                        .resources
                        .get(&GraphResourceId(r.resource_id.clone()))
                        .cloned()
                        .unwrap_or_default();
                    if r.size != first.size || s.samples != first_shape.samples {
                        errors.push(error("graph.attachment_mismatch", &p.pass_id));
                    }
                }
            }
            for id in &p.color_targets {
                if aspect(id) != GraphAspect::Color {
                    errors.push(error("graph.color_format", format!("{}:{id}", p.pass_id)));
                }
            }
            if let Some(id) = &p.depth_target {
                if aspect(id) != GraphAspect::Depth {
                    errors.push(error("graph.depth_format", format!("{}:{id}", p.pass_id)));
                }
            }
            for a in &access {
                let id = &a.resource.0;
                let Some(r) = resources.get(id) else {
                    errors.push(error(
                        "graph.missing_resource",
                        format!("{}:{id}", p.pass_id),
                    ));
                    continue;
                };
                let shape = self
                    .contracts
                    .resources
                    .get(&a.resource)
                    .cloned()
                    .unwrap_or_default();
                if a.mip >= shape.mip_levels
                    || a.layer >= shape.layers
                    || a.aspect != aspect(id)
                        && !(a.aspect == GraphAspect::Buffer
                            && r.resource_kind == RenderResourceKind::Buffer)
                {
                    errors.push(error(
                        "graph.invalid_subresource",
                        format!("{}:{id}", p.pass_id),
                    ));
                    continue;
                }
                let key = (id.clone(), a.mip, a.layer);
                uses.entry(id.clone()).or_default().push(i);
                if access
                    .iter()
                    .filter(|b| b.resource == a.resource && b.mip == a.mip && b.layer == a.layer)
                    .count()
                    > 1
                    && access.iter().any(|b| {
                        b.resource == a.resource
                            && b.mip == a.mip
                            && b.layer == a.layer
                            && b.kind != GraphAccessKind::Read
                    })
                {
                    errors.push(error("graph.feedback", format!("{}:{id}", p.pass_id)));
                }
                let read = a.kind != GraphAccessKind::Write;
                let write = a.kind != GraphAccessKind::Read;
                if read
                    && write
                    && access.iter().any(|b| {
                        b.resource == a.resource
                            && b.mip == a.mip
                            && b.layer == a.layer
                            && b.kind == GraphAccessKind::Read
                    })
                {
                    errors.push(error("graph.feedback", format!("{}:{id}", p.pass_id)));
                }
                if read && !initialized.contains(&key) {
                    errors.push(error(
                        "graph.uninitialized_read",
                        format!("{}:{id}", p.pass_id),
                    ));
                }
                if let Some(&w) = last_write.get(&key) {
                    if w != i {
                        edges.insert((w, i));
                    }
                }
                if write {
                    for &reader in readers.entry(key.clone()).or_default().iter() {
                        if reader != i {
                            edges.insert((reader, i));
                        }
                    }
                    readers.remove(&key);
                    last_write.insert(key.clone(), i);
                    initialized.insert(key);
                } else {
                    readers.entry(key).or_default().insert(i);
                }
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let Some(order) = topo(self.passes.len(), &edges) else {
            return Err(vec![error("graph.cycle", &self.graph_id)]);
        };
        let positions = order
            .iter()
            .enumerate()
            .map(|(pos, &i)| (i, pos))
            .collect::<BTreeMap<_, _>>();
        let mut lifetimes = Vec::new();
        for (id, u) in uses {
            let first = u.iter().map(|i| positions[i]).min().unwrap();
            let last = u.iter().map(|i| positions[i]).max().unwrap();
            lifetimes.push(GraphResourceLifetimeRange {
                resource: GraphResourceId(id),
                first,
                last,
                transient_slot: None,
            });
        }
        lifetimes.sort_by_key(|l| l.first);
        let mut slots: Vec<(String, usize)> = Vec::new();
        for l in &mut lifetimes {
            let r = resources[&l.resource.0];
            if r.lifetime != RenderResourceLifetime::FrameLocal {
                continue;
            }
            let shape = self
                .contracts
                .resources
                .get(&l.resource)
                .cloned()
                .unwrap_or_default();
            let slot = slots.iter().position(|(id, last)| {
                let other = resources[id];
                let s = self
                    .contracts
                    .resources
                    .get(&GraphResourceId(id.clone()))
                    .cloned()
                    .unwrap_or_default();
                *last < l.first
                    && r.size == other.size
                    && r.format == other.format
                    && r.resource_kind == other.resource_kind
                    && r.usage == other.usage
                    && shape.mip_levels == s.mip_levels
                    && shape.layers == s.layers
                    && shape.samples == s.samples
            });
            let index = slot.unwrap_or(slots.len());
            if index == slots.len() {
                slots.push((l.resource.0.clone(), l.last));
            } else {
                slots[index] = (l.resource.0.clone(), l.last);
            }
            l.transient_slot = Some(index as u32);
        }
        Ok(CompiledGraphSchedule {
            resources: self.resources.clone(),
            resource_shapes: self.contracts.resources.clone(),
            order,
            dependencies: edges.into_iter().collect(),
            lifetimes,
            history,
        })
    }
}
