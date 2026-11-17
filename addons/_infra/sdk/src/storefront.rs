//! Catalog-only presentation metadata: installing a package never requires loading its UI.
use crate::Result;
use serde::{Serialize,Deserialize};
use std::collections::BTreeMap;
pub type Text=BTreeMap<String,String>;
pub type Paragraphs=BTreeMap<String,Vec<String>>;
#[derive(Clone,Debug,Default,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presentation {
 #[serde(default)]pub summary:Text,
 #[serde(default)]pub description:Paragraphs,
 #[serde(default)]pub requirements:Paragraphs,
 #[serde(default)]pub icon:Option<String>,
 #[serde(default)]pub screenshots:Vec<Screenshot>,
 #[serde(default)]pub links:Vec<Link>,
 #[serde(default)]pub changes:Vec<Release>,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Screenshot{pub image:String,#[serde(default)]pub caption:Text}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link{pub url:String,pub label:Text}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Release{pub version:String,pub notes:Paragraphs}
pub fn localized<'a,T>(values:&'a BTreeMap<String,T>,language:&str)->Option<&'a T>{values.get(language).or_else(||values.get(language.split('-').next().unwrap_or(language))).or_else(||values.get("en")).or_else(||values.values().next())}
pub fn url(value:&str)->Result<()>{
 if !value.starts_with("https://")||value.len()>2048||value.chars().any(|c|c.is_whitespace()||c.is_control())||value[8..].split('/').next().unwrap_or("").contains('@')||value[8..].split('/').next().unwrap_or("").is_empty(){return Err("Invalid storefront HTTPS link".into());}Ok(())
}
pub fn media(value:&str)->Result<()>{
 if let Some(glyph)=value.strip_prefix("glyph:"){return crate::manifest::identifier(glyph);}
 if let Some(asset)=value.strip_prefix("asset:"){crate::manifest::relative_file(asset)?;if asset.len()>240{return Err("Oversized storefront asset".into());}Ok(())}else{url(value)}
}
impl Presentation{
 pub fn validate(&self)->Result<()>{
  fn text(v:&Text)->Result<()>{if v.len()>24||v.iter().any(|(k,v)|k.is_empty()||k.len()>32||v.len()>12000||v.contains('\0')){Err("Invalid localized storefront text".into())}else{Ok(())}}
  fn paragraphs(v:&Paragraphs)->Result<()>{if v.len()>24{return Err("Too many storefront languages".into());}for(k,lines)in v{if lines.len()>32{return Err("Too many storefront paragraphs".into());}text(&BTreeMap::from([(k.clone(),lines.join("\n"))]))?;}Ok(())}
  text(&self.summary)?;paragraphs(&self.description)?;paragraphs(&self.requirements)?;
  if let Some(icon)=&self.icon{media(icon)?;}
  if self.screenshots.len()>12||self.links.len()>12||self.changes.len()>12{return Err("Too many storefront resources".into());}
  for screenshot in &self.screenshots{media(&screenshot.image)?;text(&screenshot.caption)?;}
  for link in &self.links{url(&link.url)?;text(&link.label)?;}
  for release in &self.changes{semver::Version::parse(&release.version).map_err(|e|e.to_string())?;paragraphs(&release.notes)?;}
  Ok(())
 }
}
