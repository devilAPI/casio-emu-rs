#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include "src/context.h"
#include "src/core/core.h"
#include "src/core/mem.h"
#include "src/core/regs.h"
static uint8_t *code_mem,*data_mem,*emu_kb,*sfr;
static void setup(struct u8_sim_ctx *ctx){
    FILE *f=fopen("rom.bin","rb"); fseek(f,0,SEEK_END); long n=ftell(f); fseek(f,0,SEEK_SET);
    code_mem=malloc(n); fread(code_mem,1,n,f); fclose(f);
    data_mem=calloc(0x8000,1); emu_kb=calloc(0x30,1); sfr=calloc(0x1000,1);
    ctx->core.mem.num_regions=7; ctx->core.mem.regions=malloc(sizeof(struct u8_mem_reg)*7);
    ctx->core.mem.regions[0]=(struct u8_mem_reg){U8_REGION_BOTH,false,0x00000,0x07FFF,U8_MACC_ARR,{.array=code_mem}};
    ctx->core.mem.regions[1]=(struct u8_mem_reg){U8_REGION_DATA,true,0x08000,0x08DFF,U8_MACC_ARR,{.array=data_mem}};
    ctx->core.mem.regions[2]=(struct u8_mem_reg){U8_REGION_DATA,true,0x08E00,0x08E2F,U8_MACC_ARR,{.array=emu_kb}};
    ctx->core.mem.regions[3]=(struct u8_mem_reg){U8_REGION_DATA,true,0x0F000,0x0FFFF,U8_MACC_ARR,{.array=sfr}};
    ctx->core.mem.regions[4]=(struct u8_mem_reg){U8_REGION_CODE,false,0x08000,0x0FFFF,U8_MACC_ARR,{.array=code_mem+0x8000}};
    ctx->core.mem.regions[5]=(struct u8_mem_reg){U8_REGION_BOTH,false,0x10000,0x1FFFF,U8_MACC_ARR,{.array=code_mem+0x10000}};
    ctx->core.mem.regions[6]=(struct u8_mem_reg){U8_REGION_DATA,false,0x80000,0x8FFFF,U8_MACC_ARR,{.array=code_mem}};
}
static void render(struct u8_core *core,uint8_t *out){
    for(int y=0;y<8;y++) for(int x=0;x<48;x++) for(int Py=0;Py<4;Py++){
        uint16_t off=(y<<6)+(Py<<4)+(x>>2);
        uint8_t byte=read_mem_data(core,0,0xf800+off,1);
        for(int Px=0;Px<2;Px++) out[(y*4+Py)*96+x*2+Px]=(byte>>(7-(((x&3)<<1)+Px)))&1;
    }
}
int main(int argc,char**argv){
    if(argc<4){ fprintf(stderr,"usage: %s STEPS KI KO\n",argv[0]); return 2; }
    long steps=atol(argv[1]); uint8_t ki=(uint8_t)strtol(argv[2],0,0), ko=(uint8_t)strtol(argv[3],0,0);
    struct u8_sim_ctx ctx; memset(&ctx,0,sizeof(ctx)); setup(&ctx); u8_reset(&ctx.core);
    uint8_t last_ready=0; int sent=0;
    for(long i=0;i<steps;i++){
        uint8_t ready=read_mem_data(&ctx.core,0,0x8e00,1);
        if(last_ready==0 && ready==1){ write_mem_data(&ctx.core,0,0x8e01,1,0); write_mem_data(&ctx.core,0,0x8e02,1,0); }
        if(i>=2000000 && !sent && ready){ write_mem_data(&ctx.core,0,0x8e01,1,ki); write_mem_data(&ctx.core,0,0x8e02,1,ko); sent=1; }
        last_ready=ready;
        u8_step(&ctx.core);
    }
    uint8_t out[96*32]; render(&ctx.core,out);
    printf("pc=%04x csr=%02x psw=%02x\n",ctx.core.regs.pc,ctx.core.regs.csr,ctx.core.regs.psw);
    for(int y=0;y<32;y++){for(int x=0;x<96;x++) putchar(out[y*96+x]?'#':'.'); putchar('\n');}
    return 0;
}
