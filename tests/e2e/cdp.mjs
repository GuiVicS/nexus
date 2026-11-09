// Mini driver CDP (sem dependências). Node >= 22 (WebSocket global).
import {spawn} from 'node:child_process';
import http from 'node:http';
export async function launch({url,w=1920,h=1080,port=9333+Math.floor(Math.random()*500),mobile=false}){
  const ch=spawn('chromium',['--headless=new','--no-sandbox','--disable-gpu','--autoplay-policy=no-user-gesture-required',`--remote-debugging-port=${port}`,`--window-size=${w},${h}`,'--user-data-dir=/tmp/cdp-'+port,'about:blank'],{stdio:'ignore'});
  const get=u=>new Promise((res,rej)=>http.get(u,r=>{let d='';r.on('data',c=>d+=c);r.on('end',()=>res(JSON.parse(d)))}).on('error',rej));
  let tabs;for(let i=0;i<60;i++){try{tabs=await get(`http://127.0.0.1:${port}/json`);if(tabs.length)break}catch{}await new Promise(r=>setTimeout(r,250))}
  const ws=new WebSocket(tabs.find(t=>t.type==='page').webSocketDebuggerUrl);
  await new Promise(r=>ws.onopen=r);
  let id=0;const pend=new Map(),errors=[],logs=[];
  ws.onmessage=m=>{const d=JSON.parse(m.data);
    if(d.id&&pend.has(d.id)){pend.get(d.id)(d);pend.delete(d.id)}
    else if(d.method==='Runtime.exceptionThrown'){const e=d.params.exceptionDetails;errors.push(`${e.exception?.description||e.text} @${e.lineNumber}:${e.columnNumber}`)}
    else if(d.method==='Runtime.consoleAPICalled'&&['error','warning'].includes(d.params.type))logs.push(d.params.type+': '+d.params.args.map(a=>a.value||a.description).join(' '));
    else if(d.method==='Log.entryAdded'&&d.params.entry.level==='error')logs.push('log: '+d.params.entry.text+' '+(d.params.entry.url||''));
  };
  const send=(method,params={})=>new Promise(res=>{const i=++id;pend.set(i,res);ws.send(JSON.stringify({id:i,method,params}))});
  await send('Runtime.enable');await send('Page.enable');await send('Log.enable');
  await send('Emulation.setDeviceMetricsOverride',{width:w,height:h,deviceScaleFactor:1,mobile});
  await send('Page.navigate',{url});
  await new Promise(r=>setTimeout(r,1500));
  const api={
    errors,logs,send,
    async ev(expr){const r=await send('Runtime.evaluate',{expression:expr,returnByValue:true,awaitPromise:true});if(r.result?.exceptionDetails)throw new Error('eval: '+(r.result.exceptionDetails.exception?.description||r.result.exceptionDetails.text));return r.result?.result?.value},
    async key(k,{n=1,wait=120}={}){const map={ArrowUp:38,ArrowDown:40,ArrowLeft:37,ArrowRight:39,Enter:13,Escape:27,Tab:9,' ':32};
      for(let i=0;i<n;i++){const o={key:k,code:k.length===1?'Key'+k.toUpperCase():k,windowsVirtualKeyCode:map[k]||k.toUpperCase().charCodeAt(0)};
        await send('Input.dispatchKeyEvent',{type:'keyDown',...o});await send('Input.dispatchKeyEvent',{type:'keyUp',...o});await new Promise(r=>setTimeout(r,wait))}},
    async wait(ms){await new Promise(r=>setTimeout(r,ms))},
    async shot(path){const r=await send('Page.captureScreenshot',{format:'png'});(await import('node:fs')).writeFileSync(path,Buffer.from(r.result.data,'base64'))},
    async click(x,y){for(const t of ['mouseMoved','mousePressed','mouseReleased'])await send('Input.dispatchMouseEvent',{type:t,x,y,button:'left',clickCount:1});},
    close(){try{ws.close()}catch{}ch.kill('SIGKILL')}
  };
  return api;
}
