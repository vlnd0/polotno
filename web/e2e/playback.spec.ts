import {test,expect} from '@playwright/test';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
const manifest=JSON.parse(readFileSync(fileURLToPath(new URL('../public/wallpapers/manifest.json',import.meta.url)),'utf8'));
const clip=manifest.find((item:{id:number})=>item.id===1);
const video=readFileSync(fileURLToPath(new URL('../public/wallpapers/'+clip.file,import.meta.url)));
const image=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a0ioAAAAASUVORK5CYII=','base64');

test('video frames advance while static/shared textures upload only once and paused scene stops drawing',async({page})=>{
  await page.addInitScript(()=>{
    const stats={images:0,videos:0,draws:0};(window as any).__previewStats=stats;
    const prototype=WebGLRenderingContext.prototype;
    for(const name of ['texImage2D','texSubImage2D'] as const){
      const original=prototype[name];
      (prototype as any)[name]=function(...args:any[]){const source=args[args.length-1];if(source instanceof HTMLImageElement)stats.images++;if(source instanceof HTMLVideoElement||(source instanceof HTMLCanvasElement&&source.dataset.videoTexture))stats.videos++;return (original as any).apply(this,args)};
    }
    const draw=prototype.drawArrays;prototype.drawArrays=function(...args){stats.draws++;return draw.apply(this,args)};
  });
  const region=(id:string,asset:string)=>{const index=id==='video-a'?0:id==='video-b'?1:Number(id.split('-')[1])+2,x=.02+(index%4)*.24,y=.02+Math.floor(index/4)*.24;return {id,name:id,corners:[{x,y},{x:x+.21,y},{x:x+.21,y:y+.21},{x,y:y+.21}],playlist:{items:[asset],interval_seconds:60,muted:true}}};
  await page.route('**/api/**',async route=>{
    const path=new URL(route.request().url()).pathname;
    if(path==='/api/media/image')return route.fulfill({contentType:'image/png',body:image});
    if(path==='/api/media/video'){
      const range=route.request().headers().range?.match(/bytes=(\d+)-(\d*)/);
      const start=range?Number(range[1]):0,end=range?.[2]?Number(range[2]):video.length-1;
      return route.fulfill({status:range?206:200,contentType:'video/mp4',headers:{'accept-ranges':'bytes',...(range?{'content-range':`bytes ${start}-${end}/${video.length}`}:{})},body:video.subarray(start,end+1)});
    }
    let data:unknown;
    if(path==='/api/state')data={scene:{id:'scene',name:'Видео и картины',surfaces:[region('video-a','video'),region('video-b','video'),...Array.from({length:12},(_,i)=>region('painting-'+i,'image'))]},assets:[{id:'image',kind:'image',name:'Картина'},{id:'video',kind:'video',name:'Видео'}],revision:1,paused:false};
    else if(path==='/api/catalog')data={items:[],page:1,has_more:false};else data=[];
    return route.fulfill({contentType:'application/json',body:JSON.stringify(data)});
  });
  await page.goto('/');await expect.poll(()=>page.evaluate(()=>(window as any).__previewStats.videos)).toBeGreaterThan(25);
  const before=await page.evaluate(()=>(window as any).__previewStats);
  expect(before.images).toBe(1);await page.waitForTimeout(600);
  const after=await page.evaluate(()=>(window as any).__previewStats);expect(after.images).toBe(1);expect(after.videos).toBeGreaterThan(before.videos+5);
  await page.getByRole('button',{name:'Ⅱ Пауза'}).click();await page.waitForTimeout(200);
  const paused=await page.evaluate(()=>(window as any).__previewStats.draws);await page.waitForTimeout(300);
  expect(await page.evaluate(()=>(window as any).__previewStats.draws)).toBe(paused);
});
