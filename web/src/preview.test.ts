import {it,expect,vi} from 'vitest';
import {PreviewQueue} from './preview';
import type {Scene} from './types';
const scene=(name:string):Scene=>({id:'scene',name,surfaces:[]});
it('serializes in-flight edits and flushes the latest drag before save',async()=>{
  let release:()=>void=()=>{};const sent:string[]=[];
  const send=vi.fn(async(s:Scene)=>{sent.push(s.name);if(s.name==='first')await new Promise<void>(resolve=>release=resolve)});
  const queue=new PreviewQueue(send,()=>{});queue.enqueue(scene('first'));const initial=queue.flush();await Promise.resolve();
  queue.enqueue(scene('discarded'));queue.enqueue(scene('latest'));const flushed=queue.flush();
  expect(sent).toEqual(['first']);release();await Promise.all([initial,flushed]);expect(sent).toEqual(['first','latest']);queue.dispose();
});
it('streams previews during a continuous drag instead of waiting for pointer release',async()=>{
  vi.useFakeTimers();const send=vi.fn(async(_scene:Scene)=>{}),queue=new PreviewQueue(send,()=>{});
  queue.enqueue(scene('a'));await vi.advanceTimersByTimeAsync(20);queue.enqueue(scene('b'));await vi.advanceTimersByTimeAsync(20);
  expect(send).toHaveBeenCalledTimes(1);expect(send.mock.calls[0]).toEqual([scene('b')]);
  queue.enqueue(scene('c'));await vi.advanceTimersByTimeAsync(20);queue.enqueue(scene('d'));await vi.advanceTimersByTimeAsync(20);
  expect(send).toHaveBeenCalledTimes(2);queue.dispose();vi.useRealTimers();
});
