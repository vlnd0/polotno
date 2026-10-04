import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {test,expect} from '@playwright/test';
import type {Scene,State} from '../src/types';
const image=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a0ioAAAAASUVORK5CYII=','base64');
test('choose a catalog painting, drag region/corner, duplicate, save, and reload',async({page})=>{
  const scene:Scene={id:'00000000-0000-4000-8000-000000000001',name:'Новая сцена',surfaces:[]};
  const asset={id:'00000000-0000-4000-8000-000000000002',name:'Спальня',kind:'image',mime:'image/png',bytes:image.length};
  let state={scene,assets:[],revision:1,paused:false} as State;
  let saved:Scene[]=[structuredClone(scene)];
  const errors:string[]=[];page.on('pageerror',error=>errors.push(error.message));
  await page.route('**/api/**',async route=>{
    const request=route.request(),path=new URL(request.url()).pathname;
    if(path.startsWith('/api/media/'))return route.fulfill({contentType:'image/png',body:image});
    let data:unknown;
    if(path==='/api/state')data=state;
    else if(path==='/api/scenes')data=saved;
    else if(path==='/api/catalog')data={items:[{id:28560,title:'Спальня',artist:'Винсент ван Гог',date:'1889',preview_url:'data:image/png;base64,'+image.toString('base64'),source_url:'https://www.artic.edu/artworks/28560',rights:'CC0'}],page:1,has_more:false};
    else if(path==='/api/catalog/import'){state={...state,assets:[asset] as State['assets']};data=asset}
    else if(path==='/api/preview'||path==='/api/save'){state={...state,scene:request.postDataJSON(),revision:state.revision+1};if(path==='/api/save')saved=[structuredClone(state.scene)];data=state}
    else data={};
    return route.fulfill({contentType:'application/json',body:JSON.stringify(data)});
  });
  await page.goto('/');await page.getByRole('button',{name:'Показать',exact:true}).click();
  await expect(page.locator('[data-surface]')).toHaveCount(1);
  await expect.poll(()=>state.scene.surfaces.length).toBe(1);
  expect(state.scene.surfaces[0].playlist.items).toEqual([asset.id]);
  await page.locator('svg[aria-label]').scrollIntoViewIfNeeded();
  const box=(await page.locator('svg[aria-label]').boundingBox())!;
  await page.mouse.move(box.x+box.width*.4,box.y+box.height*.4);await page.mouse.down();await page.mouse.move(box.x+box.width*.5,box.y+box.height*.5,{steps:8});await page.mouse.up();
  await expect.poll(()=>state.scene.surfaces[0].corners[0].x).toBeCloseTo(.3,2);
  const handle=page.locator('[data-corner="0"]');const corner=(await handle.boundingBox())!;
  await page.mouse.move(corner.x+corner.width/2,corner.y+corner.height/2);await page.mouse.down();await page.mouse.move(corner.x+corner.width/2+20,corner.y+corner.height/2+12,{steps:6});await page.mouse.up();
  await expect.poll(()=>state.scene.surfaces[0].corners[0].x).toBeGreaterThan(.31);
  await page.getByRole('button',{name:'Дублировать',exact:true}).click();await expect(page.locator('[data-surface]')).toHaveCount(2);
  await page.getByRole('button',{name:'Сохранить сцену',exact:true}).click();await expect(page.getByRole('button',{name:'Сохранено',exact:true})).toBeVisible();
  expect(saved[0].surfaces).toHaveLength(2);expect(saved[0].surfaces[0].id).not.toBe(saved[0].surfaces[1].id);
  await page.reload();await expect(page.locator('[data-surface]')).toHaveCount(2);expect(errors).toEqual([]);
});

test('bare address supports a projector code and accepts a stale QR for an already paired browser',async({page})=>{
  let paired=false,calls=0;
  await page.route('**/api/**',async route=>{
    const path=new URL(route.request().url()).pathname;
    if(path==='/api/pair'){
      calls++;if(route.request().postDataJSON().code!=='123456')return route.fulfill({status:401,contentType:'application/json',body:JSON.stringify({error:'Код устарел. Покажите новый на проекторе'})});
      paired=true;return route.fulfill({contentType:'application/json',body:'{"paired":true}'});
    }
    if(!paired)return route.fulfill({status:401,contentType:'application/json',body:'{"error":"Нужно сопряжение"}'});
    const value=path==='/api/state'?{scene:{id:'scene',name:'Моя сцена',surfaces:[]},assets:[],paused:false,revision:1}:path==='/api/catalog'?{items:[],page:1,has_more:false}:[];
    return route.fulfill({contentType:'application/json',body:JSON.stringify(value)});
  });
  await page.goto('/');await expect(page.getByRole('heading',{name:'Подключите этот браузер'})).toBeVisible();
  await page.getByRole('textbox',{name:'Код подключения'}).fill('111111');await page.getByRole('button',{name:'Подключиться',exact:true}).click();
  await expect(page.getByRole('status')).toContainText('Код устарел');
  await page.getByRole('textbox',{name:'Код подключения'}).fill('123456');await page.getByRole('button',{name:'Подключиться',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Ваша стена'})).toBeVisible();expect(calls).toBe(2);
  await page.goto('/#pair=already-consumed');await page.reload();await expect(page.getByRole('heading',{name:'Ваша стена'})).toBeVisible();expect(calls).toBe(2);expect(new URL(page.url()).hash).toBe('');
});

test('network failure shows a connection error instead of asking for QR',async({page})=>{
  await page.route('**/api/**',route=>route.abort('connectionrefused'));
  await page.goto('/');await expect(page.getByRole('heading',{name:'Проектор недоступен'})).toBeVisible();
  await expect(page.getByRole('button',{name:'Попробовать снова'})).toBeVisible();await expect(page.getByRole('textbox',{name:'Код подключения'})).toHaveCount(0);
});

test('scene cards open saved compositions in one click and create an empty scene',async({page})=>{
  const first={id:'first',name:'Стена',surfaces:[]},second={id:'second',name:'Две картины',surfaces:[{id:'surface',name:'Картина',corners:[{x:.1,y:.1},{x:.4,y:.1},{x:.4,y:.4},{x:.1,y:.4}],playlist:{items:[],interval_seconds:60,muted:true}}]};let current=first;
  await page.route('**/api/**',async route=>{
    const path=new URL(route.request().url()).pathname;let data:unknown;
    if(path==='/api/preview')current=route.request().postDataJSON();
    if(path==='/api/scenes')data=[first,second];else if(path==='/api/catalog')data={items:[],page:1,has_more:false};else data={scene:current,assets:[],revision:1,paused:false};
    return route.fulfill({contentType:'application/json',body:JSON.stringify(data)});
  });
  await page.goto('/');await expect(page.getByRole('combobox')).toHaveCount(0);
  await page.getByRole('button',{name:'Открыть сцену Две картины'}).click();await expect(page.locator('[data-surface]')).toHaveCount(1);
  await page.getByRole('button',{name:'Новая сцена',exact:true}).click();await expect(page.locator('[data-surface]')).toHaveCount(0);
  await expect.poll(()=>current.name).toBe('Новая сцена');
});

test('dynamic wallpaper choices and cyberpunk are immediately available without searching',async({page})=>{
  await page.addInitScript(()=>{const play=HTMLMediaElement.prototype.play;HTMLMediaElement.prototype.play=function(){(window as any).__previewVideoStarted=true;return play.call(this)}});
  const items=[{id:1,provider:'polotno',title:'Кибергород',artist:'Полотно',date:'8 с',category:'Киберпанк',kind:'video',preview_url:'data:image/png;base64,'+image.toString('base64'),video_url:'/api/wallpapers/1/video'},{id:2,provider:'polotno',title:'Северное сияние',artist:'Полотно',date:'8 с',category:'Спокойные',kind:'video',preview_url:'data:image/png;base64,'+image.toString('base64'),video_url:'/api/wallpapers/2/video'}];
  await page.route('**/api/**',async route=>{const path=new URL(route.request().url()).pathname;const data=path==='/api/state'?{scene:{id:'scene',name:'Стена',surfaces:[]},assets:[],revision:1,paused:false}:path==='/api/catalog'?{items,page:1,has_more:false}:[];return route.fulfill({contentType:'application/json',body:JSON.stringify(data)})});
  await page.goto('/');await expect(page.getByRole('button',{name:'Динамические',exact:true})).toBeVisible();
  await expect(page.getByRole('heading',{name:'Кибергород',exact:true})).toBeVisible();await expect(page.getByRole('heading',{name:'Северное сияние',exact:true})).toBeVisible();
  await expect(page.getByRole('textbox',{name:'Поиск картин'})).toHaveCount(0);
  await page.getByRole('button',{name:'Киберпанк',exact:true}).click();await expect(page.getByRole('heading',{name:'Кибергород',exact:true})).toBeVisible();await expect(page.getByRole('heading',{name:'Северное сияние',exact:true})).toHaveCount(0);
});

test('switching animated thumbnails releases the old decoder instead of playing off screen',async({page})=>{
  const manifest=JSON.parse(readFileSync(fileURLToPath(new URL('../public/wallpapers/manifest.json',import.meta.url)),'utf8'));
  const data=readFileSync(fileURLToPath(new URL('../public/wallpapers/'+manifest.find((i:{id:number})=>i.id===1).file,import.meta.url)));
  await page.addInitScript(()=>{(window as any).__catalogVideos=[];document.addEventListener('playing',event=>{if(event.target instanceof HTMLVideoElement)(window as any).__catalogVideos.push(event.target)},true)});
  const items=[1,2].map(id=>({id,provider:'polotno',title:'Обои '+id,artist:'Полотно',date:'8 с',kind:'video',preview_url:'data:image/png;base64,'+image.toString('base64'),video_url:'/api/wallpapers/'+id+'/video'}));
  await page.route('**/api/**',async route=>{
    const path=new URL(route.request().url()).pathname;
    if(path.includes('/api/wallpapers/'))return route.fulfill({contentType:'video/mp4',body:data});
    const value=path==='/api/state'?{scene:{id:'scene',name:'Стена',surfaces:[]},assets:[],revision:1,paused:false}:path==='/api/catalog'?{items,page:1,has_more:false}:[];
    return route.fulfill({contentType:'application/json',body:JSON.stringify(value)});
  });
  await page.goto('/');const first=page.locator('.art-card').filter({has:page.getByRole('heading',{name:'Обои 1',exact:true})});await first.hover();
  await expect.poll(()=>page.evaluate(()=>(window as any).__catalogVideos.length)).toBeGreaterThan(0);
  const second=page.locator('.art-card').filter({has:page.getByRole('heading',{name:'Обои 2',exact:true})});await second.hover();
  await expect.poll(()=>page.evaluate(()=>(window as any).__catalogVideos[0].paused&&(window as any).__catalogVideos[0].getAttribute('src')===null)).toBe(true);
  await expect(page.locator('.art-grid video')).toHaveCount(1);
});

test('new region is a calibration grid and neon frame settings survive save/reload',async({page})=>{
  let scene:any={id:'scene',name:'Стена',surfaces:[]};let saved=structuredClone(scene);
  await page.route('**/api/**',async route=>{const path=new URL(route.request().url()).pathname;if(path==='/api/preview'||path==='/api/save'){scene=route.request().postDataJSON();if(path==='/api/save')saved=structuredClone(scene)}const data=path==='/api/scenes'?[saved]:path==='/api/catalog'?{items:[],page:1,has_more:false}:{scene,assets:[],revision:1,paused:false};return route.fulfill({contentType:'application/json',body:JSON.stringify(data)})});
  await page.goto('/');await page.getByRole('button',{name:'Добавить область'}).click();await expect(page.getByRole('checkbox',{name:'Калибровочная сетка'})).toBeChecked();
  await page.getByRole('checkbox',{name:'Неоновая рамка'}).check();await page.getByRole('button',{name:'Цвет рамки #ec4899'}).click();await page.getByRole('button',{name:'Сохранить сцену'}).click();
  await expect(page.getByRole('button',{name:'Сохранено',exact:true})).toBeVisible();
  expect(saved.surfaces[0].calibration).toBe(true);expect(saved.surfaces[0].frame.color).toBe('#ec4899');expect(saved.surfaces[0].fit).toBe('contain');
  await page.reload();await page.locator('.surface-row').first().click();await expect(page.getByRole('checkbox',{name:'Неоновая рамка'})).toBeChecked();
});

test('fourth video assignment, duplication and playlist append can be saved and reopened',async({page})=>{
  const video=readFileSync(fileURLToPath(new URL('./fixtures/portrait.mp4',import.meta.url)));
  const asset={id:'clip',name:'Ролик',kind:'video',mime:'video/mp4',bytes:video.length};
  let scene:any={id:'scene',name:'Стена',surfaces:[]},saved=structuredClone(scene);
  await page.route('**/api/**',async route=>{
    const path=new URL(route.request().url()).pathname;
    if(path==='/api/media/clip')return route.fulfill({contentType:'video/mp4',body:video});
    if(path==='/api/media/clip/poster')return route.fulfill({contentType:'image/png',body:image});
    if(path==='/api/preview'||path==='/api/save'){scene=route.request().postDataJSON();if(path==='/api/save')saved=structuredClone(scene)}
    const value=path==='/api/scenes'?[saved]:path==='/api/catalog'?{items:[],page:1,has_more:false}:{scene,assets:[asset],revision:1,paused:false};
    return route.fulfill({contentType:'application/json',body:JSON.stringify(value)});
  });
  await page.goto('/');await page.getByRole('button',{name:/^Мои файлы/}).click();
  for(let i=0;i<4;i++){
    await page.getByRole('button',{name:'Добавить область'}).click();
    await page.getByRole('button',{name:'Показать',exact:true}).click();
    await expect.poll(()=>scene.surfaces.filter((s:any)=>s.playlist.items.includes(asset.id)).length).toBe(i+1);
  }
  await page.getByRole('button',{name:'Дублировать',exact:true}).click();
  await page.getByRole('button',{name:'＋ В плейлист',exact:true}).click();
  await page.getByRole('button',{name:'Сохранить сцену'}).click();
  await expect(page.getByRole('button',{name:'Сохранено',exact:true})).toBeVisible();
  expect(saved.surfaces).toHaveLength(5);expect(saved.surfaces[4].playlist.items).toEqual(['clip','clip']);
  await page.reload();await expect(page.locator('[data-surface]')).toHaveCount(5);
});

for(const width of [1440,390])test(`playing portrait preview keeps selection accessible at ${width}px`,async({page})=>{
  await page.setViewportSize({width,height:1100});
  const video=readFileSync(fileURLToPath(new URL('./fixtures/portrait.mp4',import.meta.url)));
  const asset={id:'portrait',name:'Вертикальный ролик',kind:'video',mime:'video/mp4',bytes:video.length,width:180,height:320};
  let scene:Scene={id:'scene',name:'Стена',surfaces:[]};
  await page.route('**/api/**',async route=>{
    const path=new URL(route.request().url()).pathname;
    if(path==='/api/media/portrait')return route.fulfill({contentType:'video/mp4',body:video});
    if(path==='/api/media/portrait/poster')return route.fulfill({contentType:'image/png',body:image});
    if(path==='/api/preview')scene=route.request().postDataJSON();
    const value=path==='/api/scenes'?[]:path==='/api/catalog'?{items:[],page:1,has_more:false}:{scene,assets:[asset],revision:1,paused:false};
    return route.fulfill({contentType:'application/json',body:JSON.stringify(value)});
  });
  await page.goto('/');await page.getByRole('button',{name:/^Мои файлы/}).click();
  const card=page.locator('.art-card');await card.scrollIntoViewIfNeeded();
  await expect(card.locator('img')).toBeVisible();
  expect((await card.locator('img').boundingBox())!.height).toBe(width<720?150:210);
  // A touch-sized viewport starts playback with the preview button.
  if(width<720){await page.mouse.move(0,0);await card.getByRole('button',{name:'Предпросмотр Вертикальный ролик'}).dispatchEvent('click')}
  else await card.hover();
  await expect.poll(()=>card.locator('video').evaluate((v:HTMLVideoElement)=>v.videoHeight===320&&!v.paused&&v.currentTime>0)).toBe(true);
  const preview=(await card.locator('.art-image').boundingBox())!,media=(await card.locator('video').boundingBox())!,info=(await card.locator('.art-info').boundingBox())!;
  expect(preview.height).toBe(width<720?150:210);
  expect(media.y).toBe(preview.y);expect(media.height).toBe(preview.height);
  expect(info.y).toBeGreaterThanOrEqual(preview.y+preview.height);
  await expect(card.locator('video')).toHaveCSS('object-fit','contain');
  await card.getByRole('button',{name:'Показать',exact:true}).click();
  await expect.poll(()=>scene.surfaces[0]?.playlist.items).toEqual(['portrait']);
});
