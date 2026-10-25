#import <Foundation/Foundation.h>
void PeggleAutoPrepare(NSString *documents,BOOL reload);
void PeggleAutoGameStarted(void);
void PeggleAutoGameStopped(void);
void PeggleAutoEnable(void(^completion)(NSString *));
NSString *PeggleAutoStatus(void);
NSString *PeggleAutoStartupIssue(void);
void PeggleAutoWake(void);
void PeggleAutoBackground(void);

void PeggleAutoDisconnect(void);
