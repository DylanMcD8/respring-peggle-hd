#import <Foundation/Foundation.h>
void PeggleCloudProgress(NSDictionary *(^action)(int,NSString *,BOOL),void(^completion)(NSDictionary *));
void PeggleCloudExchange(NSDictionary *source,NSDictionary *(^pureMerge)(NSDictionary *),void(^completion)(NSDictionary *));

extern NSString * const PeggleCloudSyncDidCompleteNotification;
NSDate *PeggleCloudLastSyncDate(void);
