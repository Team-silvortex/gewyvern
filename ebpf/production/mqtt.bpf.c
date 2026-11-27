#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

/* Broker-to-client MQTT 3.1.1. Topics and application payloads are skipped. */
enum { SEQUENCE=1, MALFORMED, UNSUPPORTED, LIMIT, BUDGET, READ_FAILURE };
enum { HEADER, LENGTH, CONTROL, TOPIC_HIGH, TOPIC_LOW, SKIP_TOPIC, ID_HIGH, ID_LOW, SKIP_PAYLOAD };
struct mq_item { __u32 descriptor, extra; };
struct mq_flow {
    __u64 busy, raced;
    __u32 origin, next, port, fault, state, session;
    __u32 header, digits, length, left, topic, id, pos, extra;
    __u32 count, closed, segments, max_batch;
    struct mq_item items[32];
};
_Static_assert(sizeof(struct mq_flow)==344, "MQTT map ABI");
struct cursor { struct __sk_buff *skb; struct mq_flow *r; __u32 at, end; };

static __always_inline void emit(struct mq_flow *r) {
    __u32 i=r->count;
    if (i>=32) { r->fault=LIMIT; return; }
    __u32 desc=(r->header<<24)|r->id;
    if (r->header==0x90) desc|=(r->length-2)<<16;
    r->items[i].descriptor=desc; r->items[i].extra=r->extra;
    r->count=i+1; r->state=HEADER;
}

static __always_inline void start_body(struct mq_flow *r) {
    __u32 h=r->header, n=r->length;
    r->left=n;
    if ((h>>4)==3) {
        if (n < 3U + ((h&6) ? 2U : 0U)) r->fault=MALFORMED;
        else r->state=TOPIC_HIGH;
    } else if (h==0xd0) {
        if (n) r->fault=MALFORMED;
        else emit(r);
    } else if (h==0x90) {
        if (n<3) r->fault=MALFORMED;
        else if (n>10) r->fault=LIMIT;
        else r->state=CONTROL;
    } else if (n!=2) r->fault=MALFORMED;
    else r->state=CONTROL;
}

static long step(__u32 index, void *data) {
    struct cursor *c=data; struct mq_flow *r=c->r;
    if (c->at>=c->end || r->fault) return 1;
    if (r->state==SKIP_TOPIC || r->state==SKIP_PAYLOAD) {
        __u32 n=c->end-c->at;
        __u32 limit=r->state==SKIP_TOPIC ? r->topic : r->left;
        if (n>limit) n=limit;
        c->at+=n; r->left-=n;
        if (r->state==SKIP_TOPIC) {
            r->topic-=n;
            if (!r->topic) r->state=(r->header&6)?ID_HIGH:SKIP_PAYLOAD;
        }
        if (!r->left) emit(r);
        return 0;
    }
    __u8 ch;
    if (bpf_skb_load_bytes(c->skb,c->at,&ch,1)) { r->fault=READ_FAILURE; return 1; }
    c->at++;
    if (r->state==HEADER) {
        if ((!r->session && ch!=0x20) || (r->session && ch==0x20) || r->session==2) {
            r->fault=UNSUPPORTED; return 1;
        }
        if ((ch>>4)==3) {
            if ((ch&6)==6 || (!(ch&6) && (ch&8))) { r->fault=MALFORMED; return 1; }
        } else {
            switch (ch) {
            case 0x20: case 0x40: case 0x50: case 0x62: case 0x70:
            case 0x90: case 0xb0: case 0xd0: break;
            default: r->fault=UNSUPPORTED; return 1;
            }
        }
        r->header=ch; r->digits=r->length=r->id=r->extra=r->pos=0; r->state=LENGTH;
    } else if (r->state==LENGTH) {
        __u32 i=r->digits++;
        if (i>3 || (i==3 && (ch&128)) || (i && !ch)) { r->fault=MALFORMED; return 1; }
        r->length|=(__u32)(ch&127)<<(i*7);
        if (r->length>1024*1024) { r->fault=LIMIT; return 1; }
        if (!(ch&128)) start_body(r);
    } else {
        r->left--;
        switch (r->state) {
        case CONTROL: {
            __u32 p=r->pos++;
            if (r->header==0x20) {
                if (!p) { if (ch>1) r->fault=MALFORMED; else r->extra=ch; }
                else if (ch>5 || (ch && r->extra)) r->fault=MALFORMED;
                else { r->id=ch; r->session=ch?2:1; }
            } else if (!p) r->id=(__u32)ch<<8;
            else if (p==1) { r->id|=ch; if (!r->id) r->fault=MALFORMED; }
            else if (p<=9 && (ch<=2 || ch==128)) r->extra|=(__u32)(ch==128?3:ch)<<((p-2)*2);
            else r->fault=MALFORMED;
            break;
        }
        case TOPIC_HIGH: r->topic=(__u32)ch<<8; r->state=TOPIC_LOW; break;
        case TOPIC_LOW:
            r->topic|=ch;
            if (!r->topic || r->topic+((r->header&6)?2U:0U)>r->left) r->fault=MALFORMED;
            else { r->extra=r->left-r->topic-((r->header&6)?2U:0U); r->state=SKIP_TOPIC; }
            break;
        case ID_HIGH: r->id=(__u32)ch<<8; r->state=ID_LOW; break;
        case ID_LOW: r->id|=ch; if (!r->id) r->fault=MALFORMED; r->state=SKIP_PAYLOAD; break;
        default: r->fault=MALFORMED;
        }
        if (!r->left && !r->fault) emit(r);
    }
    return 0;
}
#define STREAM_ROW mq_flow
#define STREAM_INGRESS mqtt311_in
#define STREAM_PORT MQTT_PORT
#include "scoped_stream.inc.c"
