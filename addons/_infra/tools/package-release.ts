import {resolve,join,basename} from "node:path";
import {mkdir,copyFile,cp,writeFile,readFile} from "node:fs/promises";
import {buildAddonUi} from "./build-ui";

// Package only explicitly staged runtime files, never a developer's source tree.
const [targetDir,outputDir,tag,mode]=process.argv.slice(2);
const native=mode==="--native";
if(mode&&!native)throw new Error("Unknown package mode");
if(!targetDir||!outputDir||!tag||!/^[a-zA-Z0-9._-]+$/.test(tag))throw new Error("Usage: bun package-release.ts TARGET_DIR OUTPUT_DIR RELEASE_TAG [--native]");
const root=resolve(import.meta.dir,"../../..");
const output=resolve(outputDir);
const kit=join(resolve(targetDir),"release/peligames-addon");
const recipes=native?["optiscaler","anr"]:["optiscaler","afmf","mako","dlssnr-mochizuki","gpu-screen-recorder"];
const entries=[];
await mkdir(join(output,"packages"),{recursive:true});
for(const id of recipes){
 const source=join(root,"addons",id);
 const manifest=JSON.parse(await readFile(join(source,native?"manifest.native.json":"manifest.json"),"utf8"));
 // These artifacts are built on Linux x86_64; other architectures need their own build.
 manifest.platforms=["linux-x86_64"];
 const payload=join(output,"payload",id);
 await mkdir(join(payload,"ui"),{recursive:true});await mkdir(join(payload,"bin"),{recursive:true});
 const cargo=await readFile(join(source,"service/Cargo.toml"),"utf8");
 const crate=cargo.match(/name\s*=\s*"([^"]+)"/)?.[1];if(!crate)throw new Error(`Missing service name: ${id}`);
 const binary=join(resolve(targetDir),"release",crate);
 if(native){
  const info=Bun.spawnSync([binary,"--package-info"]);
  if(info.exitCode!==0)throw new Error(`Cannot inspect native service: ${id}`);
  const identity=JSON.parse(info.stdout.toString());
  if(identity.id!==manifest.id||identity.version!==manifest.version||identity.entrypoints.ui!==manifest.entrypoints.ui)throw new Error(`Rebuild ${id} with --features native-ui before packaging`);
 }
 await copyFile(binary,join(payload,manifest.entrypoints.service));
 if(native)await copyFile(join(source,manifest.entrypoints.ui),join(payload,manifest.entrypoints.ui));
 else await writeFile(join(payload,manifest.entrypoints.ui),await buildAddonUi(join(source,"ui/src/index.tsx")),{flag:"wx"});
 await copyFile(join(root,"LICENSE"),join(payload,"LICENSE"));
 await cp(join(root,"licenses/upstream"),join(payload,"licenses/upstream"),{recursive:true});
 await copyFile(join(root,"licenses/generated/Rust-Dependencies.txt"),join(payload,"licenses/Rust-Dependencies.txt"));
 const manifestPath=join(output,`${id}-manifest.json`);await writeFile(manifestPath,JSON.stringify(manifest,null,2));
 const file=join(output,"packages",`${id}-${manifest.version}-linux-x86_64.${native?"pg":"zip"}`);
 const result=Bun.spawnSync([kit,"pack",manifestPath,payload,file]);
 if(result.exitCode!==0)throw new Error(result.stderr.toString());
 const artifact=JSON.parse(result.stdout.toString());
 const presentation=JSON.parse(await readFile(join(source,"store.json"),"utf8"));
 const url=tag==="main"?`https://raw.githubusercontent.com/pelicanux/pg-addons/main/packages/${basename(file)}`:`https://github.com/pelicanux/pg-addons/releases/download/${tag}/${basename(file)}`;
 entries.push({manifest,presentation,artifacts:{"linux-x86_64":{url,sha256:artifact.sha256,bytes:artifact.bytes}}});
}
const catalog=join(output,"catalog.json");await writeFile(catalog,JSON.stringify({schema_version:1,addons:entries},null,2)+"\n");
const check=Bun.spawnSync([kit,"validate-catalog",catalog]);if(check.exitCode!==0)throw new Error(check.stderr.toString());
console.log(check.stdout.toString());
