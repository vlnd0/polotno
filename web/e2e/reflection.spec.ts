import {test,expect} from '@playwright/test';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';

for(const kind of ['image','video'] as const)test(`${kind} reflection swaps rendered pixels per surface and survives save/reload`,async({page})=>{
  await page.addInitScript(()=>{
    const context=HTMLCanvasElement.prototype.getContext;
    (HTMLCanvasElement.prototype as any).getContext=function(type:string,options:any){return (context as any).call(this,type,type==='webgl'?{...options,preserveDrawingBuffer:true}:options)};
  });
  const bytes=readFileSync(fileURLToPath(new URL(`./fixtures/mirror.${kind==='image'?'png':'mp4'}`,import.meta.url)));
  const region=(id:string,x:number)=>({id,name:id,corners:[{x,y:.1},{x:x+.4,y:.1},{x:x+.4,y:.9},{x,y:.9}],fit:'stretch',playlist:{items:['media'],interval_seconds:60,muted:true}});
  let scene:any={id:'scene',name:'Отражение',surfaces:[region('А',.05),region('Б',.55)]},saved=structuredClone(scene);
  const corners=structuredClone(scene.surfaces[0].corners);
  await page.route('**/api/**',async route=>{
    const path=new URL(route.request().url()).pathname;
    if(path==='/api/media/media')return route.fulfill({contentType:kind==='image'?'image/png':'video/mp4',body:bytes});
    if(path==='/api/preview'||path==='/api/save'){scene=route.request().postDataJSON();if(path==='/api/save')saved=structuredClone(scene)}
    const value=path==='/api/scenes'?[saved]:path==='/api/catalog'?{items:[],page:1,has_more:false}:{scene,assets:[{id:'media',name:'Медиа',kind}],paused:true,revision:1};
    return route.fulfill({contentType:'application/json',body:JSON.stringify(value)});
  });
  const pixels=()=>page.locator('canvas[aria-label="Предпросмотр сцены"]').evaluate((canvas:HTMLCanvasElement)=>{
    const gl=canvas.getContext('webgl')!,color=(x:number,y:number)=>{
      const p=new Uint8Array(4);gl.readPixels(Math.round(x*canvas.width),Math.round((1-y)*canvas.height),1,1,gl.RGBA,gl.UNSIGNED_BYTE,p);
      if(p[0]>180&&p[1]>180&&p[2]<80)return 'yellow';
      if(p[0]>180&&p[1]<80&&p[2]<80)return 'red';
      if(p[1]>180&&p[0]<80&&p[2]<80)return 'green';
      if(p[2]>180&&p[0]<80&&p[1]<80)return 'blue';return 'unready';
    };
    return [.05,.55].map(x=>[color(x+.1,.3),color(x+.3,.3),color(x+.1,.7),color(x+.3,.7)]);
  });
  const original=['red','green','blue','yellow'];
  await page.goto('/');await expect.poll(pixels).toEqual([original,original]);
  await page.locator('.surface-row').first().click();
  await page.getByRole('checkbox',{name:'Отразить по горизонтали'}).check();
  await expect.poll(pixels).toEqual([['green','red','yellow','blue'],original]);
  await page.getByRole('checkbox',{name:'Отразить по вертикали'}).check();
  await expect.poll(pixels).toEqual([['yellow','blue','green','red'],original]);
  await page.getByRole('checkbox',{name:'Отразить по горизонтали'}).uncheck();
  await expect.poll(pixels).toEqual([['blue','yellow','red','green'],original]);
  await page.getByRole('checkbox',{name:'Отразить по горизонтали'}).check();
  await page.getByRole('button',{name:'Сохранить сцену'}).click();
  await expect(page.getByRole('button',{name:'Сохранено',exact:true})).toBeVisible();
  expect(saved.surfaces[0].corners).toEqual(corners);expect(saved.surfaces[1].flip_horizontal).toBeFalsy();
  await page.reload();await expect.poll(pixels).toEqual([['yellow','blue','green','red'],original]);
  await page.locator('.surface-row').first().click();
  await expect(page.getByRole('checkbox',{name:'Отразить по горизонтали'})).toBeChecked();
  await expect(page.getByRole('checkbox',{name:'Отразить по вертикали'})).toBeChecked();
});
