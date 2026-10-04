import {defineConfig} from '@playwright/test';
export default defineConfig({
  testDir:'./e2e',
  workers:1,
  use:{baseURL:'http://127.0.0.1:4173',viewport:{width:1440,height:1100},launchOptions:{args:['--use-angle=swiftshader','--enable-unsafe-swiftshader']}},
  webServer:{command:'npm exec vite -- preview --host 127.0.0.1 --port 4173',url:'http://127.0.0.1:4173',reuseExistingServer:false},
  reporter:'list',
});
