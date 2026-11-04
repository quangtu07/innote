// Verify public release artifacts against the updater key already embedded in
// installed clients. Never reads the private signing key or its password.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {createPublicKey,createHash,verify}=require('node:crypto');
const version=process.argv[2]||JSON.parse(fs.readFileSync('package.json','utf8')).version,dir=path.resolve(process.argv[3]||'release');
const config=JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json','utf8'));
const manifest=JSON.parse(fs.readFileSync(path.join(dir,'latest.json'),'utf8'));
const file=path.join(dir,`Innote-Setup-${version}.exe`),bytes=fs.readFileSync(file);
const signature=fs.readFileSync(file+'.sig','utf8').trim();
assert.equal(manifest.version,version);assert.equal(config.version,version);
assert.equal(manifest.platforms['windows-x86_64'].signature,signature);
assert.equal(manifest.platforms['windows-x86_64'].url,`https://github.com/quangtu07/innote/releases/download/v${version}/Innote-Setup-${version}.exe`);
const publicLines=Buffer.from(config.plugins.updater.pubkey,'base64').toString('utf8').trim().split(/\r?\n/);
const signatureLines=Buffer.from(signature,'base64').toString('utf8').trim().split(/\r?\n/);
const pub=Buffer.from(publicLines[1],'base64'),sig=Buffer.from(signatureLines[1],'base64'),globalSig=Buffer.from(signatureLines[3],'base64');
assert.equal(pub.length,42);assert.equal(sig.length,74);assert.equal(globalSig.length,64);
assert(pub.subarray(2,10).equals(sig.subarray(2,10)),'Signing key must match existing clients');
assert(signatureLines[2].startsWith('trusted comment: '));
const key=createPublicKey({key:Buffer.concat([Buffer.from('302a300506032b6570032100','hex'),pub.subarray(10)]),format:'der',type:'spki'});
const algorithm=sig.subarray(0,2).toString('ascii');assert(['Ed','ED'].includes(algorithm));
const message=data=>algorithm==='ED'?createHash('blake2b512').update(data).digest():data;
assert(verify(null,message(bytes),key,sig.subarray(10)),'Installer signature invalid');
assert(verify(null,Buffer.concat([sig.subarray(10),Buffer.from(signatureLines[2].slice(17))]),key,globalSig),'Trusted comment signature invalid');
const tampered=Buffer.from(bytes);tampered[tampered.length-1]^=1;
assert(!verify(null,message(tampered),key,sig.subarray(10)),'Must reject a changed installer');
const result={version,bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex'),signatureValid:true,existingClientKeyMatches:true,tamperingRejected:true,manifestMatches:true};
fs.mkdirSync('test-results',{recursive:true});fs.writeFileSync(`test-results/release-${version}-verification.json`,JSON.stringify(result,null,2));console.log(JSON.stringify(result));
