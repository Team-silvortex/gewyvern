#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

enum { SEQUENCE=1, MALFORMED, UNSUPPORTED, LIMIT, BUDGET, READ_FAILURE };
enum { GREETING, AUTH, FIRST, COLUMN, META_END, ROWS, DENIED };
enum { HELLO=1, AUTH_OK, OK, AUTH_ERR, SESSION_ERR, ROW, NIL, RESULT_END, ONE_COLUMN, DEFINITION, META_EOF };
struct my_flow {
    __u64 busy, raced;
    __u32 origin, next, port, fault, phase, expected, header, length, pos, skip;
    __u32 kind, number, mask, width, count, closed, segments, max_batch, items[32];
};
_Static_assert(sizeof(struct my_flow) == 216, "MySQL flow ABI");
struct cursor { struct __sk_buff *skb; struct my_flow *r; __u32 at, end; };

static __always_inline void finish(struct my_flow *r) {
    __u32 k=r->kind, detail=0;
    switch (k) {
    case HELLO: r->phase=AUTH; r->expected=2; break;
    case ONE_COLUMN: r->phase=COLUMN; break;
    case DEFINITION: r->phase=META_END; break;
    case META_EOF: r->phase=ROWS; break;
    case ROW: detail=r->number; break;
    case NIL: break;
    case AUTH_ERR: case SESSION_ERR:
        detail=r->number | (r->mask << 16);
        r->phase=k==AUTH_ERR ? DENIED : FIRST; r->expected=1; break;
    default: r->phase=FIRST; r->expected=1; break;
    }
    if (k<=RESULT_END) {
        __u32 i=r->count;
        if (i>=32) { r->fault=LIMIT; return; }
        r->items[i]=(k<<24)|detail; r->count=i+1;
    }
    r->header=0; r->pos=0; r->skip=0;
}

static __always_inline void first_byte(struct my_flow *r, __u8 ch) {
    __u32 len=r->length, phase=r->phase;
    r->number=r->width=r->mask=0;
    if (phase==GREETING && ch==10 && len>=34) { r->kind=HELLO; r->skip=1; }
    else if ((phase==AUTH || phase==FIRST || phase==ROWS) && ch==255 && len>=9) {
        r->kind=phase==AUTH?AUTH_ERR:SESSION_ERR; r->mask=3;
    } else if ((phase==AUTH || phase==FIRST) && !ch && len==7) r->kind=phase==AUTH?AUTH_OK:OK;
    else if (phase==FIRST && ch==1 && len==1) r->kind=ONE_COLUMN;
    else if (phase==COLUMN && ch==3 && len>=22) r->kind=DEFINITION;
    else if ((phase==META_END || phase==ROWS) && ch==254 && len==5) r->kind=phase==ROWS?RESULT_END:META_EOF;
    else if (phase==ROWS && ch==251 && len==1) r->kind=NIL;
    else if (phase==ROWS && ch<=250 && len==1U+ch) {r->kind=ROW; r->number=ch; r->skip=1;}
    else if (phase==ROWS && (ch==252 || ch==253)) {r->kind=ROW; r->width=ch==252?2:3; if(len<=r->width)r->fault=MALFORMED;}
    else r->fault=UNSUPPORTED;
}

static long step(__u32 idx, void *data) {
    struct cursor *c=data; struct my_flow *r=c->r;
    if(c->at>=c->end || r->fault) return 1;
    if(r->header==4 && r->skip) {
        __u32 n=c->end-c->at, left=r->length-r->pos;
        if(n>left)n=left;
        c->at+=n; r->pos+=n;
        if(r->pos==r->length)finish(r);
        return 0;
    }
    __u8 ch;
    if(bpf_skb_load_bytes(c->skb,c->at,&ch,1)){r->fault=READ_FAILURE;return 1;}
    c->at++;
    if(r->header<4){
        switch(r->header++){
        case 0:r->length=ch;break;
        case 1:r->length|=(__u32)ch<<8;break;
        case 2:r->length|=(__u32)ch<<16;break;
        default:
            if(!r->length)r->fault=MALFORMED;
            else if(r->length>2*1024*1024)r->fault=LIMIT;
            else if(ch!=r->expected || r->phase==DENIED)r->fault=SEQUENCE;
            r->expected=(ch+1)&255;
        }
        return 0;
    }
    __u32 p=r->pos++, k=r->kind;
    if(!p)first_byte(r,ch);
    else if(k==AUTH_ERR || k==SESSION_ERR){
        if(p==1)r->number=ch;
        else if(p==2)r->number|=(__u32)ch<<8;
        else if(p==3 && (ch!='#' || !r->number))r->fault=MALFORMED;
        else if(p>=4 && p<=8){
            if(!((ch>='0' && ch<='9')||(ch>='A' && ch<='Z')))r->fault=MALFORMED;
            if(ch!="28000"[p-4])r->mask&=~1U;
            if(ch!="42000"[p-4])r->mask&=~2U;
            if(p==8)r->skip=1;
        }
    } else if(k==DEFINITION){
        if(p>3 || ch!="def"[p-1])r->fault=MALFORMED;
        if(p==3)r->skip=1;
    } else if(k==OK || k==AUTH_OK || k==RESULT_END || k==META_EOF){
        /* Only zero-count protocol-4.1 OK and classic EOF, no multiple results. */
        if((k==OK || k==AUTH_OK) && p<=2 && ch)r->fault=UNSUPPORTED;
        if(p==3 && (ch&8))r->fault=UNSUPPORTED;
    } else if(k==ROW && p<=3){
        r->number|=(__u32)ch<<((p-1)*8);
        if(p==r->width){
            if(r->number!=(r->length-1-r->width) || r->number<(p==2?251:65536))r->fault=MALFORMED;
            else r->skip=1;
        }
    } else r->fault=MALFORMED;
    if(!r->fault && r->pos==r->length)finish(r);
    return 0;
}
#define STREAM_ROW my_flow
#define STREAM_INGRESS mysql_classic_in
#define STREAM_PORT MYSQL_PORT
#include "scoped_stream.inc.c"
