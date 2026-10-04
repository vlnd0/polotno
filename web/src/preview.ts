import type {Scene} from './types';

/** Throttle live updates while dragging, keep only the latest scene and never reorder writes. */
export class PreviewQueue {
  private pending?:Scene;
  private timer?:ReturnType<typeof setTimeout>;
  private active?:Promise<void>;
  private error?:Error;
  private disposed=false;
  private notified=false;
  constructor(private send:(scene:Scene)=>Promise<unknown>,private status:(message:string)=>void) {}
  enqueue(scene:Scene){
    if(this.disposed)return;
    this.pending=scene; // Editor scenes are immutable; do not deep-clone on every pointer event.
    this.schedule();
  }
  private schedule(){
    if(this.timer||this.active||!this.pending||this.disposed)return;
    this.timer=setTimeout(()=>{this.timer=undefined;void this.run().catch(()=>{})},40);
  }
  private async run():Promise<void>{
    if(this.active)return this.active;
    const scene=this.pending;if(!scene||this.disposed)return;
    this.pending=undefined;
    if(!this.notified){this.status('Обновляем стену…');this.notified=true}
    this.active=this.send(scene).then(()=>{
      this.error=undefined;
      if(!this.pending&&!this.disposed){this.status('Показано на проекторе');this.notified=false}
    }).catch(error=>{this.error=error;if(!this.disposed)this.status(error.message);throw error}).finally(()=>{this.active=undefined;this.schedule()});
    return this.active;
  }
  async flush(){
    clearTimeout(this.timer);this.timer=undefined;
    while(this.pending||this.active){await (this.active||this.run());clearTimeout(this.timer);this.timer=undefined}
    if(this.error)throw this.error;
  }
  dispose(){this.disposed=true;clearTimeout(this.timer);this.timer=undefined;this.pending=undefined}
}
