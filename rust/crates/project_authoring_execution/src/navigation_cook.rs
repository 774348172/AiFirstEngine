//! Offline Recast bake in the ordinary assembler. Runtime only consumes cooked tiles.
use super::*;
use engine_runtime::{canonical_digest::sha256_prefixed,navigation_3d::*};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct NavigationSource {schema_version:String,asset_id:String,asset_guid:String,profile:AgentProfile3D,vertices:Vec<[f32;3]>,triangles:Vec<[u32;3]>,tiles:Vec<[i32;2]>,vertical_bounds:[f32;2],#[serde(default)]off_mesh_links:Vec<Link>}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Link {id:u32,start:[f32;3],end:[f32;3],radius:f32,bidirectional:bool}
#[cfg(not(target_arch="wasm32"))]
fn produce(document:&serde_json::Value)->Result<Vec<u8>,String>{
 let source:NavigationSource=serde_json::from_value(document.clone()).map_err(|e|e.to_string())?;source.profile.validate().map_err(|e|e.to_string())?;
 let mut seen=std::collections::BTreeSet::new();let mut links=std::collections::BTreeSet::new();
 if source.schema_version!="navigation3d-asset.v1"||source.asset_id.trim().is_empty()||source.asset_guid.trim().is_empty()||source.tiles.is_empty()||source.tiles.len()>64||source.tiles.iter().any(|t|t.iter().any(|v|v.abs()>2048)||!seen.insert(*t))||source.vertical_bounds.iter().any(|v|!v.is_finite()||v.abs()>4096.)||source.vertical_bounds[1]<=source.vertical_bounds[0]||source.off_mesh_links.iter().any(|l|!links.insert(l.id)){return Err("Invalid navigation source identity, bounds, tile or off-mesh link IDs.".into());}
 let p=&source.profile;let native_links=source.off_mesh_links.iter().map(|l|engine_navigation_native::OffMeshLink{start:l.start,end:l.end,radius:l.radius,bidirectional:u32::from(l.bidirectional),id:l.id}).collect::<Vec<_>>();
 let mut tiles=Vec::new();for [x,z] in source.tiles{
  let min=[x as f32*p.tile_width,source.vertical_bounds[0],z as f32*p.tile_width];let max=[(x+1) as f32*p.tile_width,source.vertical_bounds[1],(z+1) as f32*p.tile_width];
  let config=engine_navigation_native::BakeConfig{cell_size:p.cell_size,cell_height:p.cell_height,radius:p.radius,height:p.height,climb:p.climb,slope:p.max_slope_degrees,width:p.tile_width,min,max,x,z};
  let data=engine_navigation_native::bake(&config,&source.vertices,&source.triangles,&native_links).map_err(|e|format!("tile ({x},{z}): {e:?}"))?;tiles.push(CookedNavTile3D{x,z,data});
 }
 let output=CookedNavigation3D{schema_version:"navigation3d.v1".into(),asset_id:source.asset_id,source_digest:sha256_prefixed(&serde_json::to_vec(document).unwrap()),recipe:NAV_RECIPE.into(),profile:source.profile,tiles};output.validate().map_err(|e|e.to_string())?;serde_json::to_vec(&output).map_err(|e|e.to_string())
}
#[cfg(target_arch="wasm32")]
fn produce(_document:&serde_json::Value)->Result<Vec<u8>,String>{Err("Offline Recast Cook requires a native authoring host.".into())}
pub(super) fn cook(path:&str,document:&serde_json::Value,asset:&mut RuntimePackageSourceAsset,cache:Option<&ProjectAssemblyArtifactCache>,reports:&mut Vec<ProjectAssemblyProducerReport>)->Result<(),GameProjectCompilerError>{
 let started=std::time::Instant::now();let invalid=|e:String|assembly_error("navigation.invalid",format!("{path}: {e}"));let dependency=sha256_prefixed(&serde_json::to_vec(document).unwrap());let key=sha256_prefixed(format!("{NAV_RECIPE}|{dependency}").as_bytes());let mut report=ProjectAssemblyProducerReport::uncached("navigation-cook",0);report.producer_recipe_version=NAV_RECIPE.into();report.recipe_key=Some(key.clone());
 let cached=cache.and_then(|cache|{let lookup=cache.lookup_bytes("navigation-cook",&key,NAV_RECIPE);report.cache_status=lookup.status;report.miss_reason=lookup.reason;report.artifact_path=lookup.artifact_path.map(|p|p.display().to_string());let (Some(bytes),Some(envelope))=(lookup.artifact,lookup.envelope)else{return None;};if envelope.dependency_digest==dependency&&envelope.output_digest==sha256_prefixed(&bytes)&&serde_json::from_slice::<CookedNavigation3D>(&bytes).is_ok_and(|a|a.validate().is_ok()){Some(bytes)}else{let _=cache.quarantine("navigation-cook",&key);None}});
 let output=if let Some(bytes)=cached{bytes}else{let bytes=produce(document).map_err(&invalid)?;if let Some(cache)=cache{let published=cache.publish_bytes("navigation-cook",NAV_RECIPE,&key,&dependency,&sha256_prefixed(&bytes),&bytes).map_err(|e|invalid(format!("{e:?}")))?;report.cache_status=match published.status{crate::ProjectAssemblyArtifactPublishStatus::Produced=>crate::ProjectAssemblyArtifactCacheStatus::Produced,crate::ProjectAssemblyArtifactPublishStatus::PublishRaceReused=>crate::ProjectAssemblyArtifactCacheStatus::PublishRaceReused};report.artifact_path=Some(published.artifact_path.display().to_string());}bytes};
 report.duration_ms=started.elapsed().as_millis() as u64;report.output_digest=Some(sha256_prefixed(&output));asset.hash=report.output_digest.clone();asset.runtime_payload=Some(output);reports.push(report);Ok(())
}
