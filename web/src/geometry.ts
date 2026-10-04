import type { Point, Surface } from './types';
export const clamp = (n:number) => Math.max(0,Math.min(1,n));
export function uuid():string {
  const b=crypto.getRandomValues(new Uint8Array(16));b[6]=(b[6]&15)|64;b[8]=(b[8]&63)|128;
  const h=Array.from(b,v=>v.toString(16).padStart(2,'0')).join('');return `${h.slice(0,8)}-${h.slice(8,12)}-${h.slice(12,16)}-${h.slice(16,20)}-${h.slice(20)}`;
}
export function newSurface(index:number):Surface { return {id:uuid(),name:`Область ${index}`,corners:[{x:.2,y:.2},{x:.6,y:.2},{x:.6,y:.6},{x:.2,y:.6}],playlist:{items:[],interval_seconds:60,muted:true},frame:null,fit:'contain',calibration:true,flip_horizontal:false,flip_vertical:false} }
export function regionAspect(c:Point[],width:number,height:number):number {
  const distance=(a:Point,b:Point)=>Math.hypot((a.x-b.x)*width,(a.y-b.y)*height);
  return (distance(c[0],c[1])+distance(c[2],c[3]))/(distance(c[1],c[2])+distance(c[3],c[0]));
}
export function textureScale(mediaAspect:number,areaAspect:number,fit='contain'):number[]{
  if(fit==='stretch')return [1,1];const ratio=mediaAspect/areaAspect;
  return fit==='cover'?(ratio>1?[ratio,1]:[1,1/ratio]):(ratio>1?[1,1/ratio]:[ratio,1]);
}
export function convex(c:Point[]):boolean { return c.length===4&&c.every((a,i)=>{const b=c[(i+1)%4],d=c[(i+2)%4];return (b.x-a.x)*(d.y-b.y)-(b.y-a.y)*(d.x-b.x)>0.00001}) }
export function moveCorner(c:Point[],i:number,p:Point):Point[] { const changed=c.map((v,j)=>j===i?{x:clamp(p.x),y:clamp(p.y)}:v);return convex(changed)?changed:c }
export function translate(c:Point[],delta:Point):Point[] {
  const dx=Math.max(-Math.min(...c.map(p=>p.x)),Math.min(1-Math.max(...c.map(p=>p.x)),delta.x));
  const dy=Math.max(-Math.min(...c.map(p=>p.y)),Math.min(1-Math.max(...c.map(p=>p.y)),delta.y));
  return c.map(p=>({x:p.x+dx,y:p.y+dy}));
}
export function projection(corners:Point[]):Float32Array {
  const p=corners.map(c=>({x:2*c.x-1,y:1-2*c.y}));
  const dx1=p[1].x-p[2].x,dx2=p[3].x-p[2].x,dy1=p[1].y-p[2].y,dy2=p[3].y-p[2].y;
  const dx3=p[0].x-p[1].x+p[2].x-p[3].x,dy3=p[0].y-p[1].y+p[2].y-p[3].y;
  const det=dx1*dy2-dx2*dy1,affine=Math.abs(dx3)+Math.abs(dy3)<1e-6;
  const g=affine?0:(dx3*dy2-dx2*dy3)/det,h=affine?0:(dx1*dy3-dx3*dy1)/det;
  return new Float32Array([p[1].x-p[0].x+g*p[1].x,p[1].y-p[0].y+g*p[1].y,g,p[3].x-p[0].x+h*p[3].x,p[3].y-p[0].y+h*p[3].y,h,p[0].x,p[0].y,1]);
}
