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
static uint64_t mem_checksum(void){
    uint64_t h=0xcbf29ce484222325ULL;
    for(int i=0;i<0x8000;i++){ h ^= data_mem[i]; h *= 0x100000001b3ULL; }
    for(int i=0;i<0x1000;i++){ h ^= sfr[i]; h *= 0x100000001b3ULL; }
    for(int i=0;i<0x30;i++){ h ^= emu_kb[i]; h *= 0x100000001b3ULL; }
    return h;
}
static void hu8(uint64_t *h,uint8_t b){ *h ^= b; *h *= 0x100000001b3ULL; }
static void hu16(uint64_t *h,uint16_t v){ hu8(h,v&0xff); hu8(h,v>>8); }
static void hu64(uint64_t *h,uint64_t v){ for(int i=0;i<8;i++) hu8(h,(v>>(8*i))&0xff); }
int main(int argc,char**argv){
    struct u8_sim_ctx ctx; memset(&ctx,0,sizeof(ctx)); setup(&ctx); u8_reset(&ctx.core);
    long steps=argc>1?atol(argv[1]):10000000;
    uint64_t h=0xcbf29ce484222325ULL;
    for(long i=0;i<steps;i++){
        if(i%1000==0) { hu64(&h,mem_checksum()); printf("%ld %016llx\n",i,(unsigned long long)h); }
        struct u8_regs *r=&ctx.core.regs;
        hu16(&h,r->pc); hu8(&h,r->csr); hu8(&h,r->lcsr);
        for(int j=0;j<3;j++) hu8(&h,r->ecsr[j]);
        hu16(&h,r->lr);
        for(int j=0;j<3;j++) hu16(&h,r->elr[j]);
        hu8(&h,r->psw);
        for(int j=0;j<3;j++) hu8(&h,r->epsw[j]);
        hu16(&h,r->sp); hu16(&h,r->ea); hu8(&h,r->dsr);
        for(int j=0;j<16;j++) hu8(&h,r->gp[j]);
        u8_step(&ctx.core);
    }
    printf("done %ld\n",steps);
    return 0;
}
