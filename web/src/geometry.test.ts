import {describe,it,expect} from 'vitest';
import {moveCorner,translate,projection,convex,textureScale,newSurface} from './geometry';
const quad=[{x:.1,y:.1},{x:.8,y:.1},{x:.8,y:.8},{x:.1,y:.8}];
describe('pointer geometry',()=>{
  it('prevents crossed or collapsed corners without jumping',()=>{expect(moveCorner(quad,0,{x:.9,y:.9})).toBe(quad);expect(convex(moveCorner(quad,0,{x:.2,y:.2}))).toBe(true)});
  it('clamps whole-region movement without changing its shape',()=>{const moved=translate(quad,{x:4,y:-3});expect(moved[0].x).toBeCloseTo(.3);expect(moved[0].y).toBe(0);expect(moved[1].x-moved[0].x).toBeCloseTo(.7);expect(moved[2].y-moved[1].y).toBeCloseTo(.7)});
  it('maps all four projective corners like native GLES',()=>{const c=[{x:.1,y:.2},{x:.8,y:.1},{x:.9,y:.9},{x:.2,y:.7}],m=projection(c);[[0,0],[1,0],[1,1],[0,1]].forEach(([u,v],i)=>{const w=m[2]*u+m[5]*v+m[8];expect((m[0]*u+m[3]*v+m[6])/w).toBeCloseTo(c[i].x*2-1,5);expect((m[1]*u+m[4]*v+m[7])/w).toBeCloseTo(1-c[i].y*2,5)})});
});
it('new region starts with calibration grid and portrait containment preserves aspect',()=>{
  expect(newSurface(1).calibration).toBe(true);
  expect(textureScale(9/16,16/9,'contain')).toEqual([81/256,1]);
  expect(textureScale(9/16,16/9,'cover')).toEqual([1,256/81]);
  expect(textureScale(9/16,16/9,'stretch')).toEqual([1,1]);
});
