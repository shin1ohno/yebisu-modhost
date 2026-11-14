//! Read a frozen shell description, including the retained state it references.
use anyhow::{Context, Result, bail};
use gpui::{App, Window};
use gpui_base::ScrollbarHandle;
use gpui_shell::{
    RenderSnapshot, ScriptView,
    snapshot::{Bridged, Component, SpecOp},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

struct Capture<'a> {
    view: &'a ScriptView,
    cx: &'a App,
    window: &'a Window,
    home: &'a Path,
    output: &'a Path,
    incarnation: &'a str,
    revision: u64,
    entities: BTreeMap<u64, Value>,
    assets: BTreeMap<String, Value>,
    lists: Vec<Value>,
}
impl Capture<'_> {
    fn entity(&mut self, handle: u64, kind: &str) -> Result<Value> {
        let reference = json!({"id":handle.to_string(),"incarnation":self.incarnation,"kind":kind});
        if self.entities.contains_key(&handle) {
            return Ok(reference);
        }
        let (config, state, events) = match kind {
            "Input" => {
                let input = self
                    .view
                    .snapshot_input(handle)
                    .context("missing retained input")?;
                let input = input.read(self.cx);
                let presentation = input.presentation();
                let text = input.value().to_string();
                let (start, end, reversed) = input.snapshot_selection();
                let selection = input_selection(&text, start, end, reversed)?;
                (
                    json!({"kind":"Input","placeholder":presentation.placeholder().to_string(),"masked":presentation.is_masked(),"loading":presentation.is_loading(),"readonly":presentation.is_readonly(),"number":null}),
                    json!({"kind":"Input","text":text,"selection":selection}),
                    self.view
                        .snapshot_input_events(handle)
                        .into_iter()
                        .map(|event| event.to_lowercase())
                        .collect(),
                )
            }
            "Focus" => {
                let focus = self
                    .view
                    .snapshot_focus(handle)
                    .context("missing retained focus")?;
                (
                    json!({"kind":"Focus"}),
                    json!({"kind":"Focus","id":handle.to_string(),"focused":focus.is_focused(self.window)}),
                    vec![],
                )
            }
            "Scroll" | "VirtualScroll" => {
                let (virtual_, scroll, pending) = self
                    .view
                    .snapshot_scroll(handle)
                    .context("missing retained scroll")?;
                anyhow::ensure!(
                    virtual_ == (kind == "VirtualScroll"),
                    "scroll kind mismatch"
                );
                let offset = scroll.offset();
                let viewport = scroll.bounds().size;
                let content = scroll.content_size();
                let offset = json!({"x":f32::from(offset.x),"y":f32::from(offset.y)});
                let state = if virtual_ {
                    // Empty lists have no anchor. Nonempty row delivery is outside this prototype.
                    json!({"kind":kind,"offset":offset,"anchor_key":"","anchor_index":0,"content_rev":self.revision.to_string(),"request":null})
                } else {
                    json!({"kind":kind,"offset":offset,"viewport":{"width":f32::from(viewport.width),"height":f32::from(viewport.height)},"content":{"width":f32::from(content.width),"height":f32::from(content.height)},"request":pending.map(|index|json!({"id":format!("{handle}:{index}"),"index":index}))})
                };
                (json!({"kind":kind}), state, vec![])
            }
            _ => bail!("unsupported retained entity {kind}"),
        };
        self.entities.insert(handle,json!({"ref":reference,"rev":self.revision.to_string(),"config":config,"state":state,"subscriptions":{"rev":"1","events":events}}));
        Ok(reference)
    }

    fn argument(&mut self, name: &str, arg: &Bridged) -> Result<Value> {
        Ok(match arg {
            Bridged::Number(value)
                if matches!(
                    name,
                    "track_focus" | "content_focus_handle" | "track_scroll"
                ) =>
            {
                anyhow::ensure!(
                    value.is_finite() && *value >= 0. && value.fract() == 0.,
                    "invalid entity handle"
                );
                let handle = *value as u64;
                let kind = if name == "track_scroll" {
                    if self
                        .view
                        .snapshot_scroll(handle)
                        .context("missing scroll")?
                        .0
                    {
                        "VirtualScroll"
                    } else {
                        "Scroll"
                    }
                } else {
                    "Focus"
                };
                json!({"kind":"Entity","value":self.entity(handle,kind)?})
            }
            Bridged::Number(value) => {
                anyhow::ensure!(value.is_finite(), "nonfinite argument");
                json!({"kind":"Number","value":value})
            }
            Bridged::Str(value) => json!({"kind":"Text","value":value}),
            Bridged::Bool(value) => json!({"kind":"Bool","value":value}),
            Bridged::Nil => bail!("nil argument unsupported by wire e1: {name}"),
        })
    }
    fn asset(&mut self, path: &str) -> Result<Value> {
        let canonical = self
            .home
            .join(path)
            .canonicalize()
            .with_context(|| format!("asset {path}"))?;
        anyhow::ensure!(
            canonical.starts_with(self.home),
            "asset outside isolated home"
        );
        let bytes = std::fs::read(canonical)?;
        anyhow::ensure!(
            bytes.len() <= 512 * 1024,
            "asset exceeds prototype SVG budget"
        );
        let tree = usvg::Tree::from_data(&bytes, &usvg::Options::default())?;
        let size = tree.size().to_int_size();
        let id = digest(&bytes);
        std::fs::write(self.output.join(format!("asset-{id}.svg")), &bytes)?;
        self.assets.insert(id.clone(),json!({"id":id,"digest":id,"kind":"svg","encoded_bytes":bytes.len(),"decoded_bytes":size.width() as u64*size.height() as u64*4,"width":size.width(),"height":size.height()}));
        Ok(json!({"id":id,"digest":id}))
    }
}

fn input_selection(
    text: &str,
    start: usize,
    end: usize,
    reversed: bool,
) -> Result<yebisu_mod_wire::Selection> {
    use yebisu_mod_wire::{Direction, Selection};
    Ok(Selection::from_utf16(
        text,
        start,
        end,
        if reversed {
            Direction::Backward
        } else {
            Direction::Forward
        },
    )?)
}

// Retained native entities can notify without rebuilding their owning ScriptView.
// Compare with the last publication's revision so revision allocation itself does
// not look like a state change. Reuse the export path for config and subscriptions.
pub fn retained_changed(
    previous: &Value,
    view: &ScriptView,
    cx: &App,
    window: &Window,
    destination: Destination<'_>,
) -> Result<bool> {
    let mut cap = Capture {
        view,
        cx,
        window,
        home: destination.home,
        output: destination.output,
        incarnation: destination.incarnation,
        revision: destination.revision,
        entities: BTreeMap::new(),
        assets: BTreeMap::new(),
        lists: vec![],
    };
    for entity in previous["entities_full"]
        .as_array()
        .context("previous entities")?
    {
        let kind = entity["ref"]["kind"].as_str().context("entity kind")?;
        if kind == "View" {
            continue;
        }
        let handle = entity["ref"]["id"].as_str().context("entity id")?.parse()?;
        cap.entity(handle, kind)?;
        if cap.entities.get(&handle) != Some(entity) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub struct Destination<'a> {
    pub home: &'a Path,
    pub output: &'a Path,
    pub incarnation: &'a str,
    pub revision: u64,
}

pub fn publish(
    snapshot: &RenderSnapshot,
    view: &ScriptView,
    cx: &App,
    window: &Window,
    destination: Destination<'_>,
) -> Result<Value> {
    let Destination {
        home,
        output,
        incarnation,
        revision,
    } = destination;
    let mut cap = Capture {
        view,
        cx,
        window,
        home,
        output,
        incarnation,
        revision,
        entities: BTreeMap::new(),
        assets: BTreeMap::new(),
        lists: vec![],
    };
    // Export only reachable nodes. Detached builder scratch nodes are not part of the view.
    let mut reachable = BTreeSet::new();
    let mut pending = vec![snapshot.root()];
    while let Some(id) = pending.pop() {
        if !reachable.insert(id) {
            continue;
        }
        let node = snapshot.arena().node(id).context("invalid snapshot node")?;
        pending.extend_from_slice(node.children());
        for op in node.ops() {
            if let SpecOp::StateStyle(_, id) | SpecOp::Slot(_, id) = op {
                pending.push(*id);
            }
        }
    }
    let indices: BTreeMap<_, _> = reachable
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, i))
        .collect();
    let mut nodes = vec![];
    for id in reachable {
        let source = snapshot.arena().node(id).unwrap();
        let mut text = Value::Null;
        let mut entity = Value::Null;
        let mut asset = Value::Null;
        let mut list_id = Value::Null;
        let component = match source.component().context("missing component")? {
            Component::Div => "Div",
            Component::HFlex => "HFlex",
            Component::VFlex => "VFlex",
            Component::Text(value) => {
                text = json!(value);
                "Text"
            }
            Component::Svg(path) => {
                asset = cap.asset(path)?;
                "Svg"
            }
            Component::Input(handle) => {
                entity = cap.entity(*handle, "Input")?;
                "Input"
            }
            Component::Scrollbar(value) => {
                text = json!(value);
                "Scrollbar"
            }
            Component::VirtualList(list) => {
                anyhow::ensure!(
                    list.sizes().is_empty(),
                    "nonempty virtual rows require the later row adapter"
                );
                list_id = json!(list.id());
                cap.lists.push(json!({"list_id":list.id(),"content_rev":revision.to_string(),"kind":"virtual","item_count":list.sizes().len(),"sizes_or_estimate":{"kind":"Sizes","value":[]},"handler_generation":revision.to_string()}));
                "VirtualList"
            }
            other => bail!("unsupported prototype component: {other:?}"),
        };
        let mut ops = vec![];
        for op in source.ops() {
            ops.push(match op {
                SpecOp::NullaryStyle(index)=>json!({"op":"NullaryStyle","index":index,"name":gpui_shell::snapshot::nullary_name(*index)}),
                SpecOp::ParamStyle(name,args)|SpecOp::Method(name,args)=>json!({"op":if matches!(op,SpecOp::ParamStyle(..)){"ParamStyle"}else{"Method"},"name":name,"args":args.iter().map(|v|cap.argument(name,v)).collect::<Result<Vec<_>>>()?}),
                SpecOp::Callback(name,id)=>json!({"op":"Callback","name":name,"id":id.to_string()}),
                SpecOp::ActionCallback(name,id)=>json!({"op":"ActionCallback","name":name.to_string(),"id":id.to_string()}),
                SpecOp::StateStyle(name,id)=>json!({"op":"StateStyle","name":name,"node":indices[id]}),
                SpecOp::Slot(name,id)=>json!({"op":"Slot","name":name,"node":indices[id]}),
                SpecOp::RegisteredMethod(_)=>bail!("registered methods require a negotiated adapter"),
            });
        }
        nodes.push(json!({"component":component,"text":text,"entity":entity,"asset":asset,"text_format":null,"child_view":null,"list_id":list_id,"children":source.children().iter().map(|id|indices[id]).collect::<Vec<_>>(),"ops":ops}));
    }
    let root = indices[&snapshot.root()];
    // Shell handles originate in JS numbers; reserve a stable wire-only root id.
    let view_id = u64::MAX;
    let reference = json!({"id":view_id.to_string(),"kind":"View","incarnation":incarnation});
    cap.entities.insert(view_id,json!({"ref":reference,"rev":revision.to_string(),"config":{"kind":"View"},"state":{"kind":"View","parent":null,"root":root,"generation":revision.to_string(),"props_rev":"1"},"subscriptions":{"rev":"1","events":[]}}));
    Ok(
        json!({"pub_rev":revision.to_string(),"wm":{"processed_op_seq":"0","state_rev":revision.to_string(),"settled_local_seq":"0"},"entities_full":cap.entities.into_values().collect::<Vec<_>>(),"views_full":[{"view_ref":reference,"generation":revision.to_string(),"root":root,"nodes":nodes,"state_rev":revision.to_string()}],"focus_state":[],"overlays_full":[],"list_manifests":cap.lists,"asset_manifests":cap.assets.into_values().collect::<Vec<_>>()}),
    )
}

#[cfg(test)]
mod tests {
    use super::input_selection;
    use yebisu_mod_wire::Direction;

    #[test]
    fn selection_conversion_preserves_direction_and_rejects_surrogate_interior() {
        let selection = input_selection("あ😀é", 1, 3, true).unwrap();
        assert_eq!((selection.start, selection.end), (3, 7));
        assert_eq!(selection.direction, Direction::Backward);
        assert!(input_selection("あ😀é", 2, 3, false).is_err());
        assert!(input_selection("あ😀é", 0, 5, false).is_err());
    }
}
