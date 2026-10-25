#ifndef PEGGLE_WRAPPER_BACKING_QUERY_H
#define PEGGLE_WRAPPER_BACKING_QUERY_H
/* Shared production query sequence, also compiled by own host GL fixtures.
 * The SDL current-context drawable is independent of the persistent root view.
 * Never consume GL errors or pixels. Restore both bindings on every query path.
 */
enum { PEGGLE_BACKING_VERIFIED=1, PEGGLE_BACKING_NO_CURRENT=2,
       PEGGLE_BACKING_BIND_FAILED=3, PEGGLE_BACKING_SIZE_UNAVAILABLE=4 };
typedef struct {
 void (*bindings)(unsigned int *,unsigned int *);
 int (*bind_current)(void *);
 void (*size)(int *,int *);
 void (*restore)(unsigned int,unsigned int);
} PeggleBackingOps;
static inline int peggle_query_current_backing(void *window,int current_matches,
                                              const PeggleBackingOps *ops,
                                              unsigned int *width,unsigned int *height){
 *width=*height=0;
 if(!current_matches)return PEGGLE_BACKING_NO_CURRENT;
 unsigned int framebuffer=0,renderbuffer=0;int w=0,h=0;
 ops->bindings(&framebuffer,&renderbuffer);
 int bound=ops->bind_current(window);
 if(bound==0)ops->size(&w,&h);
 ops->restore(framebuffer,renderbuffer);
 if(bound!=0)return PEGGLE_BACKING_BIND_FAILED;
 if(w<=0||h<=0)return PEGGLE_BACKING_SIZE_UNAVAILABLE;
 *width=(unsigned int)w;*height=(unsigned int)h;return PEGGLE_BACKING_VERIFIED;
}
#endif
