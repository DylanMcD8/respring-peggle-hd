#import <UIKit/UIKit.h>
@interface PeggleRawImportPreview:UITableViewController
@property(nonatomic,strong) NSDictionary *preview;
@property(nonatomic,copy) void(^confirmImport)(void);
@property(nonatomic,copy) void(^cancelImport)(void);
@end
