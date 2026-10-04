export class ApiError extends Error { constructor(message:string,public status:number){super(message)} }
export async function api<T>(path:string,method='GET',data?:unknown):Promise<T> {
  const response=await fetch('/api/'+path,{method,headers:data===undefined?{}:{'content-type':'application/json'},body:data===undefined?undefined:JSON.stringify(data)});
  const value=await response.json().catch(()=>({error:'Не удалось прочитать ответ проектора'}));
  if(!response.ok)throw new ApiError(value.error||'Ошибка запроса',response.status);return value;
}
export async function upload(file:File) {
  if(file.size>512*1024*1024)throw Error('Файл превышает 512 МБ');
  const response=await fetch('/api/upload',{method:'POST',headers:{'content-type':file.type||'application/octet-stream','x-file-name':JSON.stringify(file.name).replace(/[\u007f-\uffff]/g,c=>'\\u'+c.charCodeAt(0).toString(16).padStart(4,'0'))},body:file});
  const value=await response.json();if(!response.ok)throw Error(value.error||'Ошибка загрузки');return value;
}
