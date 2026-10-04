export type Point = { x: number; y: number };
export type NeonFrame = {color:string;width:number;glow:number};
export type Surface = { id: string; name: string; corners: Point[]; playlist: {items:string[];interval_seconds:number;muted:boolean};frame?:NeonFrame|null;fit?:'contain'|'cover'|'stretch';calibration?:boolean;flip_horizontal?:boolean;flip_vertical?:boolean };
export type Scene = { id:string; name:string; surfaces:Surface[] };
export type Asset = {id:string;name:string;kind:'image'|'video';mime:string;bytes:number;width?:number;height?:number;original_mime?:string;attribution?:{title:string;artist:string;source_url:string;rights:string;provider_id:string}};
export type State = {scene:Scene;assets:Asset[];paused:boolean;revision:number};
export type CatalogItem = {id:number;provider:string;title:string;artist:string;date:string;preview_url:string;source_url:string;rights:string;kind?:'image'|'video';category?:string;video_url?:string};
export type CatalogPage = {items:CatalogItem[];page:number;has_more:boolean};
