#import <UIKit/UIKit.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>
#import <Foundation/Foundation.h>
#include <string.h>
#import "import_sources.h"
#import "import_preview.h"
#import "cloud_progress.h"
#import "auto_progress.h"
@interface PeggleSavePicker:UIDocumentPickerViewController
@property BOOL rawSelection;
@property BOOL progressSelection;
@property BOOL exportSelection;
@end
@implementation PeggleSavePicker
@end
extern char *peggle_save_manager_action(int,const char *,int);
extern void peggle_save_manager_free(char *);
static NSDictionary *SaveAction(int code,NSString *value,BOOL confirmed){
 char *raw=peggle_save_manager_action(code,value.UTF8String,confirmed);
 if(!raw)return @{@"ok":@NO,@"error":@"Save file management returned no result."};
 NSData *data=[NSData dataWithBytes:raw length:strlen(raw)];peggle_save_manager_free(raw);
 id result=[NSJSONSerialization JSONObjectWithData:data options:0 error:NULL];
 return [result isKindOfClass:NSDictionary.class]?result:@{@"ok":@NO,@"error":@"Save file management returned an invalid result."};
}
@interface PeggleSaveManagerController:UIViewController<UIDocumentPickerDelegate>
@property BOOL ready,busy,initialized,canRollback;
@property(nonatomic,copy) NSString *initialMessage;
@property(nonatomic,strong) NSMutableArray<UIButton *> *buttons;
@property(nonatomic,strong) UIButton *rollbackButton;
@property(copy) void(^operationCleanup)(void);
@property(nonatomic,strong) UISegmentedControl *importMode;
@property(nonatomic,strong) UISwitch *cloudSwitch;
@property(nonatomic,strong) UILabel *importDescription,*lastSynced;
@property(nonatomic,strong) UIStackView *columns,*header;
@property(nonatomic,strong) UIButton *headerPlay;
@property(nonatomic,strong) NSLayoutConstraint *playMinimumWidth,*playMinimumHeight;
@property(nonatomic,strong) UIStackView *content;
@property(nonatomic,strong) NSLayoutConstraint *contentTop,*contentBottom,*contentLeading,*contentTrailing;

@end
@implementation PeggleSaveManagerController
- (UIColor *)accent {return [UIColor colorWithRed:246.0/255 green:117.0/255 blue:39.0/255 alpha:1];}
- (UILabel *)label:(NSString *)text style:(UIFontTextStyle)style secondary:(BOOL)secondary {
 UILabel *l=[UILabel new];l.text=text;l.numberOfLines=0;l.font=[UIFont preferredFontForTextStyle:style];l.adjustsFontForContentSizeCategory=YES;l.textColor=secondary?UIColor.secondaryLabelColor:UIColor.labelColor;return l;
}
- (UIButton *)button:(NSString *)title action:(SEL)action {
 UIButtonConfiguration *c=[UIButtonConfiguration tintedButtonConfiguration];c.title=title;c.cornerStyle=UIButtonConfigurationCornerStyleCapsule;c.baseBackgroundColor=[self accent];c.baseForegroundColor=UIColor.labelColor;c.contentInsets=NSDirectionalEdgeInsetsMake(13,16,13,16);
 UIButton *b=[UIButton buttonWithConfiguration:c primaryAction:nil];if(action)[b addTarget:self action:action forControlEvents:UIControlEventTouchUpInside];b.titleLabel.adjustsFontForContentSizeCategory=YES;b.accessibilityIdentifier=title;[self.buttons addObject:b];return b;
}
- (UIButton *)playButton {
 UIButton *b=[self button:@"Play" action:@selector(continueGame)];UIButtonConfiguration *c;
 if(@available(iOS 26.0,*)){c=[UIButtonConfiguration prominentGlassButtonConfiguration];}else{c=[UIButtonConfiguration filledButtonConfiguration];}
 c.title=@"Play";c.buttonSize=UIButtonConfigurationSizeLarge;c.cornerStyle=UIButtonConfigurationCornerStyleCapsule;c.baseBackgroundColor=[self accent];c.baseForegroundColor=UIColor.blackColor;c.image=[UIImage systemImageNamed:@"play.fill"];c.imagePadding=8;b.configuration=c;self.playMinimumWidth=[b.widthAnchor constraintGreaterThanOrEqualToConstant:100];self.playMinimumHeight=[b.heightAnchor constraintGreaterThanOrEqualToConstant:50];[NSLayoutConstraint activateConstraints:@[self.playMinimumWidth,self.playMinimumHeight]];return b;
}
- (void)updatePlayMetrics {
 UIFont *font=[UIFont preferredFontForTextStyle:UIFontTextStyleHeadline compatibleWithTraitCollection:self.traitCollection];UIButtonConfiguration *c=self.headerPlay.configuration;c.titleTextAttributesTransformer=^NSDictionary *(NSDictionary *attributes){NSMutableDictionary *result=[attributes mutableCopy];result[NSFontAttributeName]=font;return [result autorelease];};c.preferredSymbolConfigurationForImage=[UIImageSymbolConfiguration configurationWithFont:font];c.contentInsets=NSDirectionalEdgeInsetsMake(12,20,12,20);self.headerPlay.configuration=c;
 self.playMinimumWidth.constant=ceil([@"Play" sizeWithAttributes:@{NSFontAttributeName:font}].width+font.lineHeight+48);self.playMinimumHeight.constant=ceil(font.lineHeight+24);
}
- (void)traitCollectionDidChange:(UITraitCollection *)previous {
 [super traitCollectionDidChange:previous];if(![previous.preferredContentSizeCategory isEqual:self.traitCollection.preferredContentSizeCategory])[self updatePlayMetrics];
}
- (UIStackView *)section:(NSString *)title symbol:(NSString *)symbol description:(NSString *)description {
 UIStackView *card=[UIStackView new];card.axis=UILayoutConstraintAxisVertical;card.spacing=12;card.layoutMarginsRelativeArrangement=YES;card.directionalLayoutMargins=NSDirectionalEdgeInsetsMake(20,20,20,20);card.backgroundColor=UIColor.secondarySystemGroupedBackgroundColor;card.layer.cornerRadius=24;card.layer.cornerCurve=kCACornerCurveContinuous;
 UIImageView *glyph=[[UIImageView alloc]initWithImage:[UIImage systemImageNamed:symbol]];glyph.tintColor=[self accent];glyph.contentMode=UIViewContentModeScaleAspectFit;[glyph.widthAnchor constraintEqualToConstant:26].active=YES;[glyph.heightAnchor constraintEqualToConstant:26].active=YES;
 UILabel *headingTitle=[self label:title style:UIFontTextStyleTitle2 secondary:NO];headingTitle.font=[[UIFontMetrics metricsForTextStyle:UIFontTextStyleTitle2]scaledFontForFont:[UIFont systemFontOfSize:22 weight:UIFontWeightSemibold]];
 UIStackView *heading=[[UIStackView alloc]initWithArrangedSubviews:@[glyph,headingTitle]];heading.spacing=10;heading.alignment=UIStackViewAlignmentCenter;[card addArrangedSubview:heading];
 [card addArrangedSubview:[self label:description style:UIFontTextStyleSubheadline secondary:YES]];return card;
}
- (void)viewDidLoad {
 [super viewDidLoad];self.view.backgroundColor=UIColor.systemGroupedBackgroundColor;self.view.tintColor=[self accent];self.buttons=[NSMutableArray new];
 UIScrollView *scroll=[UIScrollView new];scroll.translatesAutoresizingMaskIntoConstraints=NO;scroll.alwaysBounceVertical=YES;scroll.clipsToBounds=NO;scroll.backgroundColor=UIColor.clearColor;scroll.contentInsetAdjustmentBehavior=UIScrollViewContentInsetAdjustmentAutomatic;[self.view addSubview:scroll];
 UIStackView *content=[UIStackView new];content.axis=UILayoutConstraintAxisVertical;content.spacing=20;content.alignment=UIStackViewAlignmentFill;content.distribution=UIStackViewDistributionEqualSpacing;content.clipsToBounds=NO;content.backgroundColor=UIColor.clearColor;self.content=content;content.translatesAutoresizingMaskIntoConstraints=NO;[scroll addSubview:content];
 UIImageView *icon=[[UIImageView alloc]initWithImage:[UIImage imageNamed:@"ManageSavesIcon"]];icon.contentMode=UIViewContentModeScaleAspectFit;icon.isAccessibilityElement=NO;[icon.widthAnchor constraintEqualToConstant:68].active=YES;[icon.heightAnchor constraintEqualToConstant:68].active=YES;
 UILabel *title=[self label:@"Manage Save Files" style:UIFontTextStyleLargeTitle secondary:NO];title.font=[[UIFontMetrics metricsForTextStyle:UIFontTextStyleLargeTitle]scaledFontForFont:[UIFont systemFontOfSize:32 weight:UIFontWeightBold]];
 NSString *app=[NSBundle.mainBundle objectForInfoDictionaryKey:@"CFBundleDisplayName"]?:@"Peggle";
 UIStackView *titles=[[UIStackView alloc]initWithArrangedSubviews:@[[self label:app style:UIFontTextStyleSubheadline secondary:YES],title]];titles.axis=UILayoutConstraintAxisVertical;titles.spacing=2;
 UIStackView *identity=[[UIStackView alloc]initWithArrangedSubviews:@[icon,titles]];identity.spacing=16;identity.alignment=UIStackViewAlignmentCenter;
 self.headerPlay=[self playButton];[self updatePlayMetrics];[self.headerPlay setContentHuggingPriority:UILayoutPriorityRequired forAxis:UILayoutConstraintAxisHorizontal];
 self.header=[[UIStackView alloc]initWithArrangedSubviews:@[identity,self.headerPlay]];self.header.spacing=20;self.header.alignment=UIStackViewAlignmentCenter;[content addArrangedSubview:self.header];
 UILabel *hint=[self label:[NSString stringWithFormat:@"To show or hide this menu, go to Settings → Apps → %@ and toggle Show Save File Management on Launch.",app] style:UIFontTextStyleFootnote secondary:YES];[content addArrangedSubview:hint];
 self.columns=[UIStackView new];self.columns.spacing=18;self.columns.axis=UILayoutConstraintAxisHorizontal;self.columns.distribution=UIStackViewDistributionFillEqually;self.columns.alignment=UIStackViewAlignmentFill;[content addArrangedSubview:self.columns];
 UIStackView *imports=[self section:@"Import" symbol:@"square.and.arrow.down" description:@"Import save files from another device. You can review the incoming changes before applying."];
 self.importMode=[[UISegmentedControl alloc]initWithItems:@[@"Overwrite Files",@"Merge Changes"]];self.importMode.selectedSegmentIndex=1;self.importMode.selectedSegmentTintColor=[self accent];[self.importMode setTitleTextAttributes:@{NSForegroundColorAttributeName:UIColor.blackColor} forState:UIControlStateSelected];[self.importMode addTarget:self action:@selector(importModeChanged) forControlEvents:UIControlEventValueChanged];self.importMode.accessibilityIdentifier=@"Import Mode";[imports addArrangedSubview:self.importMode];
 self.importDescription=[self label:@"" style:UIFontTextStyleFootnote secondary:YES];[imports addArrangedSubview:self.importDescription];[self importModeChanged];
 [imports addArrangedSubview:[self button:@"Choose Files" action:@selector(chooseRawFiles)]];[imports addArrangedSubview:[self button:@"Choose Folder" action:@selector(chooseRawFolder)]];[imports addArrangedSubview:[self button:@"Choose ZIP" action:@selector(chooseBackup)]];[self.columns addArrangedSubview:imports];
 UIStackView *exports=[self section:@"Export" symbol:@"square.and.arrow.up" description:@"Save a copy of your in-game users, completed levels, scores, unfinished games, and replays to Files."];
 [exports addArrangedSubview:[self button:@"Save as Folder" action:@selector(exportFolder)]];[exports addArrangedSubview:[self button:@"Save as ZIP" action:@selector(exportBackup)]];
 [exports addArrangedSubview:[self label:@"Game Center data and app settings are not included in exported save files." style:UIFontTextStyleFootnote secondary:YES]];
 self.rollbackButton=[self button:@"Restore Previous Save Files…" action:@selector(rollback)];[exports addArrangedSubview:self.rollbackButton];[self.columns addArrangedSubview:exports];
 UIStackView *cloud=[self section:@"iCloud (Experimental)" symbol:@"icloud" description:@"Sync Adventure checkpoints, cleared and aced levels, and challenge completion between Respring's versions of Peggle and Peggle HD."];
 self.lastSynced=[self label:@"" style:UIFontTextStyleFootnote secondary:YES];self.lastSynced.accessibilityIdentifier=@"Last Synced";[cloud insertArrangedSubview:self.lastSynced atIndex:1];[self updateLastSynced];
 [NSNotificationCenter.defaultCenter addObserver:self selector:@selector(cloudSyncDidComplete:) name:PeggleCloudSyncDidCompleteNotification object:nil];
 self.cloudSwitch=[UISwitch new];self.cloudSwitch.onTintColor=[self accent];self.cloudSwitch.accessibilityLabel=@"Enable iCloud Sync";[self.cloudSwitch addTarget:self action:@selector(cloudChanged) forControlEvents:UIControlEventValueChanged];
 UIStackView *toggle=[[UIStackView alloc]initWithArrangedSubviews:@[[self label:@"Enable iCloud Sync" style:UIFontTextStyleBody secondary:NO],self.cloudSwitch]];toggle.spacing=12;toggle.alignment=UIStackViewAlignmentCenter;[cloud addArrangedSubview:toggle];
 [cloud addArrangedSubview:[self label:@"Automatic sync links one in-game user on this device. Linking a different user combines their completed levels and trophies with the same iCloud save file; it does not create a separate file for that user." style:UIFontTextStyleFootnote secondary:YES]];
 [cloud addArrangedSubview:[self button:@"How iCloud Sync Works" action:@selector(showCloudHelp)]];
 [cloud addArrangedSubview:[self button:@"Sync Immediately" action:@selector(syncProgress)]];
 UIButton *more=[self button:@"Import + Export Save Files" action:NULL];[more removeTarget:self action:NULL forControlEvents:UIControlEventTouchUpInside];more.showsMenuAsPrimaryAction=YES;more.menu=[UIMenu menuWithChildren:@[[UIAction actionWithTitle:@"Export Save Files" image:[UIImage systemImageNamed:@"square.and.arrow.up"] identifier:nil handler:^(UIAction *a){[self exportProgress];}],[UIAction actionWithTitle:@"Import Save Files" image:[UIImage systemImageNamed:@"square.and.arrow.down"] identifier:nil handler:^(UIAction *a){[self chooseProgress];}]]];[cloud addArrangedSubview:more];[self.columns addArrangedSubview:cloud];
 // Give extra column height to empty space below the controls, not to labels or buttons.
 for(UIStackView *card in @[imports,exports,cloud]){UIView *spacer=[UIView new];[spacer.heightAnchor constraintGreaterThanOrEqualToConstant:0].active=YES;[card addArrangedSubview:spacer];[card setCustomSpacing:0 afterView:card.arrangedSubviews[card.arrangedSubviews.count-2]];}
 // Match School Assistant's New/Edit Schoolwork layout: the content guide
 // owns vertical scrolling; the frame guide owns width. Unclipped scrolling
 // continues drawing through the bottom safe area without a hard edge.
 self.contentTop=[content.topAnchor constraintEqualToAnchor:scroll.contentLayoutGuide.topAnchor];self.contentBottom=[content.bottomAnchor constraintEqualToAnchor:scroll.contentLayoutGuide.bottomAnchor];self.contentLeading=[content.leadingAnchor constraintEqualToAnchor:scroll.frameLayoutGuide.leadingAnchor];self.contentTrailing=[content.trailingAnchor constraintEqualToAnchor:scroll.frameLayoutGuide.trailingAnchor];
 [NSLayoutConstraint activateConstraints:@[[scroll.topAnchor constraintEqualToAnchor:self.view.topAnchor],[scroll.bottomAnchor constraintEqualToAnchor:self.view.safeAreaLayoutGuide.bottomAnchor],[scroll.leadingAnchor constraintEqualToAnchor:self.view.leadingAnchor],[scroll.trailingAnchor constraintEqualToAnchor:self.view.trailingAnchor],self.contentTop,self.contentBottom,self.contentLeading,self.contentTrailing]];
 [self updateContentMargins];
 [self updateButtons];
}
- (void)updateLastSynced {
 NSDate *date=PeggleCloudLastSyncDate();
 if(!date){self.lastSynced.text=@"Last synced: Never";return;}
 NSDateFormatter *format=[NSDateFormatter new];format.dateStyle=NSDateFormatterMediumStyle;format.timeStyle=NSDateFormatterShortStyle;
 self.lastSynced.text=[NSString stringWithFormat:@"Last synced: %@",[format stringFromDate:date]];[format release];
}
- (void)cloudSyncDidComplete:(NSNotification *)notification {[self updateLastSynced];}
- (void)dealloc {[NSNotificationCenter.defaultCenter removeObserver:self];[super dealloc];}
- (void)updateContentMargins {
 NSDirectionalEdgeInsets margins=self.view.directionalLayoutMargins;CGFloat padding=MIN(20,MIN(margins.leading,margins.trailing));if(padding<=0)padding=20;
 self.content.spacing=padding;self.contentTop.constant=padding;self.contentBottom.constant=-padding;self.contentLeading.constant=margins.leading;self.contentTrailing.constant=-margins.trailing;
}
- (void)viewLayoutMarginsDidChange {[super viewLayoutMarginsDidChange];[self updateContentMargins];}
- (void)viewDidLayoutSubviews {
 [super viewDidLayoutSubviews];BOOL compact=self.view.safeAreaLayoutGuide.layoutFrame.size.width<900||UIContentSizeCategoryIsAccessibilityCategory(self.traitCollection.preferredContentSizeCategory);
 self.columns.axis=compact?UILayoutConstraintAxisVertical:UILayoutConstraintAxisHorizontal;self.columns.distribution=compact?UIStackViewDistributionFill:UIStackViewDistributionFillEqually;self.columns.alignment=UIStackViewAlignmentFill;
 BOOL narrow=self.view.safeAreaLayoutGuide.layoutFrame.size.width<500||UIContentSizeCategoryIsAccessibilityCategory(self.traitCollection.preferredContentSizeCategory);
 self.header.axis=narrow?UILayoutConstraintAxisVertical:UILayoutConstraintAxisHorizontal;self.header.alignment=narrow?UIStackViewAlignmentLeading:UIStackViewAlignmentCenter;
}
- (void)importModeChanged {self.importDescription.text=self.importMode.selectedSegmentIndex==0?@"Fully replace any matching files with the imported versions.":@"Combine Adventure checkpoints, cleared and aced levels, and challenge trophies with those of your selected in-game user. Scores and unfinished shots stay on this device. (Recommended)";}
- (void)dismissCloudHelp {[self dismissViewControllerAnimated:YES completion:nil];}
- (void)showCloudHelp {
 UIViewController *help=[UIViewController new];help.title=@"How iCloud Sync Works";help.view.backgroundColor=UIColor.systemGroupedBackgroundColor;
 help.navigationItem.rightBarButtonItem=[[UIBarButtonItem alloc]initWithTitle:@"Dismiss" style:UIBarButtonItemStyleDone target:self action:@selector(dismissCloudHelp)];
 UIScrollView *scroll=[UIScrollView new];scroll.translatesAutoresizingMaskIntoConstraints=NO;scroll.alwaysBounceVertical=YES;[help.view addSubview:scroll];
 UIStackView *body=[UIStackView new];body.translatesAutoresizingMaskIntoConstraints=NO;body.axis=UILayoutConstraintAxisVertical;body.spacing=12;[scroll addSubview:body];
 NSArray<NSArray<NSString *> *> *sections=@[
  @[@"One shared iCloud save file",@"Respring’s Peggle and Peggle HD use one shared iCloud save file for the Apple Account signed into iCloud. This file contains Adventure checkpoints, cleared and aced levels, and challenge trophies for both Classic and Nights."],
  @[@"Linking an in-game user",@"Enabling automatic sync links the in-game user currently selected in Peggle on this device. Each device can link a user with a different name. User names stay local and do not identify separate iCloud save files. Other local users do not get their own iCloud save file."],
  @[@"Combining completed levels and trophies",@"Syncing keeps the furthest Adventure checkpoint and combines cleared levels, aced levels, and challenge trophies. Linking a different user combines that user’s completed levels and trophies with the same shared file. Sync Immediately always uses the currently selected user, even if automatic sync is linked to someone else. Resets and deletions do not remove anything from the shared file."],
  @[@"When changes are applied",@"After Peggle saves, automatic sync uploads the linked user’s changes and checks for changes from your other devices. Incoming changes are applied when you open the game with the linked user selected. If they arrive during play, choose Apply and Restart or Later. Later keeps you playing and applies the changes next time you open Peggle. A later Adventure checkpoint replaces an older unfinished board and starts that level from the beginning. If iCloud is unavailable, you can keep playing; syncing retries automatically."],
  @[@"What stays on this device",@"Scores, statistics, replay files, unfinished shots, in-game user names, and app settings do not sync through iCloud. This shared save file is not a full backup. Use Export to make a folder or ZIP copy of your local save files, including all in-game users and unfinished games."]
 ];
 for(NSArray<NSString *> *section in sections){UILabel *heading=[self label:section[0] style:UIFontTextStyleHeadline secondary:NO];[body addArrangedSubview:heading];[body addArrangedSubview:[self label:section[1] style:UIFontTextStyleBody secondary:NO]];[body setCustomSpacing:24 afterView:body.arrangedSubviews.lastObject];}
 [NSLayoutConstraint activateConstraints:@[[scroll.topAnchor constraintEqualToAnchor:help.view.safeAreaLayoutGuide.topAnchor],[scroll.bottomAnchor constraintEqualToAnchor:help.view.bottomAnchor],[scroll.leadingAnchor constraintEqualToAnchor:help.view.leadingAnchor],[scroll.trailingAnchor constraintEqualToAnchor:help.view.trailingAnchor],[body.topAnchor constraintEqualToAnchor:scroll.contentLayoutGuide.topAnchor constant:20],[body.bottomAnchor constraintEqualToAnchor:scroll.contentLayoutGuide.bottomAnchor constant:-20],[body.leadingAnchor constraintEqualToAnchor:scroll.frameLayoutGuide.leadingAnchor constant:20],[body.trailingAnchor constraintEqualToAnchor:scroll.frameLayoutGuide.trailingAnchor constant:-20]]];
 UINavigationController *navigation=[[UINavigationController alloc]initWithRootViewController:help];navigation.modalPresentationStyle=UIModalPresentationFormSheet;navigation.preferredContentSize=CGSizeMake(600,700);[self presentPanel:navigation];
}
- (void)cloudChanged {if(self.cloudSwitch.on){[self enableAutomaticSync];}else{[NSUserDefaults.standardUserDefaults setBool:NO forKey:@"PeggleAutomaticProgressSync"];[self updateButtons];[self message:@"iCloud Sync is disabled. Local save files are not affected." title:@"iCloud Sync Disabled"];}}
- (void)updateButtons {BOOL enabled=self.initialized&&!self.busy&&!self.ready;for(UIButton *button in self.buttons)button.enabled=enabled;self.rollbackButton.enabled=enabled&&self.canRollback;self.importMode.enabled=enabled;self.cloudSwitch.enabled=enabled;self.cloudSwitch.on=[NSUserDefaults.standardUserDefaults boolForKey:@"PeggleAutomaticProgressSync"];}
- (void)presentPanel:(UIViewController *)panel completion:(void(^)(void))completion {
 void(^show)(void)=^{[self presentViewController:panel animated:YES completion:completion];};
 if(self.presentedViewController){[self dismissViewControllerAnimated:YES completion:show];}else{show();}
}
- (void)presentPanel:(UIViewController *)panel {[self presentPanel:panel completion:nil];}
- (void)viewDidAppear:(BOOL)animated {
 [super viewDidAppear:animated];[self updateLastSynced];
 if(self.initialMessage.length){NSString *message=self.initialMessage;self.initialMessage=nil;[self message:message title:self.initialized?@"iCloud Sync":@"Unable to Complete"];}
}
- (void)beginCloudOperation:(NSString *)message operation:(void(^)(void))operation {
 self.busy=YES;[self updateButtons];
 UIAlertController *alert=[UIAlertController alertControllerWithTitle:@"iCloud Sync" message:message preferredStyle:UIAlertControllerStyleAlert];
 // Start only after presentation finishes, including dismissal of the
 // confirmation alert. Fast completions can then replace this alert safely.
 [self presentPanel:alert completion:operation];
}
- (void)message:(NSString *)message title:(NSString *)title {
 UIAlertController *alert=[UIAlertController alertControllerWithTitle:title message:message preferredStyle:UIAlertControllerStyleAlert];[alert addAction:[UIAlertAction actionWithTitle:@"Dismiss" style:UIAlertActionStyleDefault handler:nil]];[self presentPanel:alert];
}
- (void)showError:(NSDictionary *)result {[self message:result[@"error"]?:@"The operation could not be completed. Your existing save files have not been modified." title:@"Unable to Complete"] ;}
- (void)runOperation:(NSDictionary *(^)(void))operation completion:(void(^)(NSDictionary *))completion {
 if(self.busy||self.ready)return;void(^cleanup)(void)=self.operationCleanup;self.operationCleanup=nil;self.busy=YES;[self updateButtons];
 dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED,0),^{
  NSDictionary *result=nil;@try{result=operation();}@catch(NSException *exception){result=@{@"ok":@NO,@"error":@"The selected files could not be read. Your existing save files have not been modified."};}
  dispatch_async(dispatch_get_main_queue(),^{if(cleanup)cleanup();self.busy=NO;if([result[@"ok"]boolValue]){if(result[@"can_rollback"])self.canRollback=[result[@"can_rollback"]boolValue];completion(result);}else{[self showError:result];}[self updateButtons];});
 });
}
- (void)run:(int)code value:(NSString *)value confirmed:(BOOL)confirmed completion:(void(^)(NSDictionary *))completion {
 [self runOperation:^{return SaveAction(code,value?:@"",confirmed);} completion:completion];
}
- (void)continueGame {
 [self run:5 value:@"" confirmed:NO completion:^(NSDictionary *result){self.ready=YES;[NSNotificationCenter.defaultCenter removeObserver:self];[self updateButtons];}];
}
- (void)exportAsFolder:(BOOL)folder {
 [self run:folder?14:1 value:@"" confirmed:NO completion:^(NSDictionary *result){
  NSDateFormatter *formatter=[NSDateFormatter new];formatter.locale=[NSLocale localeWithLocaleIdentifier:@"en_US_POSIX"];formatter.dateFormat=@"yyyy-MM-dd HH.mm";
  NSString *staging=[NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];NSError *error=nil;NSFileManager *files=NSFileManager.defaultManager;
  if(![files createDirectoryAtPath:staging withIntermediateDirectories:YES attributes:nil error:&error]){[self message:error.localizedDescription title:@"Unable to Export"];return;}
  NSString *name=[NSString stringWithFormat:@"Peggle Save Files — %@%@",[formatter stringFromDate:NSDate.date],folder?@"":@".zip"];NSString *friendly=[staging stringByAppendingPathComponent:name];
  if(![files copyItemAtPath:result[@"path"] toPath:friendly error:&error]){[self message:error.localizedDescription title:@"Unable to Export"];return;}
  PeggleSavePicker *picker=[[PeggleSavePicker alloc]initForExportingURLs:@[[NSURL fileURLWithPath:friendly]] asCopy:YES];picker.exportSelection=YES;picker.delegate=self;[self presentPanel:picker];
 }];
}
- (void)exportFolder {[self exportAsFolder:YES];}
- (void)exportBackup {[self exportAsFolder:NO];}
- (void)enableAutomaticSync {
 NSDictionary *source=SaveAction(10,@"",NO);if(![source[@"ok"]boolValue]){[self showError:source];[self updateButtons];return;}
 UIAlertController *a=[UIAlertController alertControllerWithTitle:@"Enable Automatic iCloud Syncing?" message:[NSString stringWithFormat:@"Link in-game user \"%@\" to the shared iCloud save file?\n\nAdventure checkpoints, cleared and aced levels, and challenge trophies will sync automatically. Peggle and Peggle HD use the same file for your Apple Account. User names can differ between devices. Linking another user combines their completed levels and trophies with this file; it does not create a separate file.\n\nThe app will open directly into the game. You can turn automatic sync off or show this menu again in Settings.",source[@"profile"]] preferredStyle:UIAlertControllerStyleAlert];
 [a addAction:[UIAlertAction actionWithTitle:@"Cancel" style:UIAlertActionStyleCancel handler:^(UIAlertAction *x){[self updateButtons];}]];
 [a addAction:[UIAlertAction actionWithTitle:@"Enable Automatic iCloud Syncing" style:UIAlertActionStyleDefault handler:^(UIAlertAction *x){[self beginCloudOperation:@"Enabling automatic iCloud sync…" operation:^{PeggleAutoEnable(^(NSString *status){self.busy=NO;[self updateButtons];[self message:status title:@"iCloud Sync Enabled"];});}];}]];
 [self presentPanel:a];
}
- (void)syncProgress {
 [self run:10 value:@"" confirmed:NO completion:^(NSDictionary *source){
  NSString *message=[NSString stringWithFormat:@"Combine Adventure checkpoints, cleared and aced levels, and challenge trophies for in-game user \"%@\" with the shared iCloud save file?\n\nThis uses your currently selected user, even if automatic sync is linked to a different user. There is one shared file for your Apple Account, so only sync users whose completed levels and trophies you want to combine.\n\nA later Adventure checkpoint replaces an older unfinished board and starts that level from the beginning. Scores and statistics stay local. Resets and deletions do not sync.",source[@"profile"]];
  UIAlertController *alert=[UIAlertController alertControllerWithTitle:@"Sync Save Files" message:message preferredStyle:UIAlertControllerStyleAlert];
  [alert addAction:[UIAlertAction actionWithTitle:@"Cancel" style:UIAlertActionStyleCancel handler:nil]];
  [alert addAction:[UIAlertAction actionWithTitle:@"Merge Save Files" style:UIAlertActionStyleDefault handler:^(UIAlertAction *action){
   [self beginCloudOperation:@"Syncing save files with iCloud…" operation:^{
   PeggleCloudProgress(^NSDictionary *(int code,NSString *value,BOOL confirmed){return SaveAction(code,value,confirmed);},^(NSDictionary *result){dispatch_async(dispatch_get_main_queue(),^{
    self.busy=NO;if([result[@"ok"]boolValue]){self.canRollback=[SaveAction(6,@"",NO)[@"can_rollback"]boolValue];[self message:[NSString stringWithFormat:@"Adventure checkpoints, cleared and aced levels, and challenge trophies synced for in-game user %@.",result[@"profile"]] title:@"iCloud Sync Complete"];}else{[self showError:result];}[self updateButtons];
   });});
   }];
  }]];[self presentPanel:alert];
 }];
}
- (void)exportProgress {
 [self run:10 value:@"" confirmed:NO completion:^(NSDictionary *result){
  
  UIActivityViewController *share=[[UIActivityViewController alloc]initWithActivityItems:@[[NSURL fileURLWithPath:result[@"path"]]] applicationActivities:nil];share.completionWithItemsHandler=^(UIActivityType activity,BOOL completed,NSArray *items,NSError *error){if(error){[self message:error.localizedDescription title:@"Unable to Export"];}else{[self message:completed?@"Your save file was exported.":@"No save file was shared." title:completed?@"Export Complete":@"Export Cancelled"];}};share.popoverPresentationController.sourceView=self.view;share.popoverPresentationController.sourceRect=CGRectMake(CGRectGetMidX(self.view.bounds),CGRectGetMidY(self.view.bounds),1,1);[self presentPanel:share];
 }];
}
- (void)chooseProgress {if(self.busy||self.ready)return;PeggleSavePicker *picker=[[PeggleSavePicker alloc]initForOpeningContentTypes:@[UTTypeJSON] asCopy:YES];picker.progressSelection=YES;picker.delegate=self;picker.allowsMultipleSelection=NO;[self presentPanel:picker];}
- (void)presentProgressPreview:(NSDictionary *)result {
 NSString *message=[NSString stringWithFormat:@"Combine Adventure checkpoints, cleared and aced levels, and challenge trophies from in-game user \"%@\" with those of your selected in-game user \"%@\"?\n\nAfter merging: %@ challenge trophies, %@ cleared levels and %@ aced levels. A later Adventure checkpoint replaces an older unfinished board and starts that level from the beginning. Scores and statistics stay local. Previous save files remain available for rollback.",result[@"source_profile"],result[@"profile"],result[@"after"][@"trophies"],result[@"after"][@"cleared"],result[@"after"][@"aced"]];
 UIAlertController *alert=[UIAlertController alertControllerWithTitle:@"Merge Save Files" message:message preferredStyle:UIAlertControllerStyleAlert];
 [alert addAction:[UIAlertAction actionWithTitle:@"Cancel" style:UIAlertActionStyleCancel handler:^(UIAlertAction *action){[self run:7 value:@"" confirmed:NO completion:^(NSDictionary *r){[self message:@"Your save files are unchanged." title:@"Import Cancelled"];}];}]];
 [alert addAction:[UIAlertAction actionWithTitle:@"Merge Save Files" style:UIAlertActionStyleDefault handler:^(UIAlertAction *action){
  if(![result[@"changed"]boolValue]){[self run:7 value:@"" confirmed:NO completion:^(NSDictionary *r){[self message:@"These Adventure checkpoints, completed levels, and trophies are already in your save files. Your save files are unchanged." title:@"Already Up to Date"];}];return;}
  [self run:12 value:result[@"token"] confirmed:YES completion:^(NSDictionary *r){[self message:@"Save files merged. Adventure will resume at your furthest checkpoint. Cleared and aced levels and challenge trophies have been combined." title:@"Import Complete"];}];
 }]];[self presentPanel:alert];
}
- (void)chooseBackup {if(self.busy||self.ready)return;PeggleSavePicker *picker=[[PeggleSavePicker alloc]initForOpeningContentTypes:@[UTTypeZIP] asCopy:YES];picker.rawSelection=NO;picker.delegate=self;picker.allowsMultipleSelection=NO;[self presentPanel:picker];}
- (void)chooseRawFiles {if(self.busy||self.ready)return;PeggleSavePicker *picker=[[PeggleSavePicker alloc]initForOpeningContentTypes:@[UTTypeData] asCopy:YES];picker.rawSelection=YES;picker.allowsMultipleSelection=YES;picker.delegate=self;[self presentPanel:picker];}
- (void)chooseRawFolder {if(self.busy||self.ready)return;PeggleSavePicker *picker=[[PeggleSavePicker alloc]initForOpeningContentTypes:@[UTTypeFolder] asCopy:NO];picker.rawSelection=YES;picker.allowsMultipleSelection=NO;picker.delegate=self;[self presentPanel:picker];}
- (void)presentRawPreview:(NSDictionary *)result {
 PeggleRawImportPreview *preview=[[PeggleRawImportPreview alloc]initWithStyle:UITableViewStyleInsetGrouped];preview.preview=result;
 UINavigationController *navigation=[[UINavigationController alloc]initWithRootViewController:preview];navigation.modalPresentationStyle=UIModalPresentationFormSheet;navigation.preferredContentSize=CGSizeMake(640,720);navigation.modalInPresentation=YES;
 preview.cancelImport=^{[navigation dismissViewControllerAnimated:YES completion:^{[self run:7 value:@"" confirmed:NO completion:^(NSDictionary *done){[self message:@"Your save files are unchanged." title:@"Import Cancelled"];}];}];};
 preview.confirmImport=^{[navigation dismissViewControllerAnimated:YES completion:^{[self run:9 value:result[@"token"] confirmed:YES completion:^(NSDictionary *done){[self message:@"The reviewed files were imported. You can restore the previous set of save files from Export if needed." title:@"Import Complete"];}];}];};
 [self presentPanel:navigation];
}
- (void)documentPicker:(UIDocumentPickerViewController *)controller didPickDocumentsAtURLs:(NSArray<NSURL *> *)urls {
 if([(PeggleSavePicker *)controller exportSelection]){[self message:@"Your save file backup was exported to the selected location." title:@"Export Complete"];return;}
 if(self.busy||self.ready)return;for(NSURL *url in urls)if(!url.isFileURL){[self message:@"Choose files or a folder from Files." title:@"Unable to Import"];return;}
 dispatch_block_t cleanup=nil;@try{cleanup=PeggleAcquireImportScopes(urls);}@catch(NSException *exception){[self message:@"The selected provider could not grant read access." title:@"Unable to Import"];return;}
 if(!cleanup){[self message:@"Select up to 128 files, or a folder of save files." title:@"Unable to Import"];return;}self.operationCleanup=cleanup;
 BOOL progress=[controller isKindOfClass:PeggleSavePicker.class]&&[(PeggleSavePicker *)controller progressSelection];
 BOOL raw=[controller isKindOfClass:PeggleSavePicker.class]&&[(PeggleSavePicker *)controller rawSelection];BOOL replace=self.importMode.selectedSegmentIndex==0;BOOL merge=!replace;
 [self runOperation:^{return PeggleCoordinateImport(urls,0,@[],^NSDictionary *(NSArray<NSString *> *paths){
  if(progress)return SaveAction(11,paths.firstObject,NO);
  if(merge){NSData *request=[NSJSONSerialization dataWithJSONObject:@{@"paths":paths,@"archive":@(!raw)} options:0 error:NULL];return SaveAction(15,[[NSString alloc]initWithData:request encoding:NSUTF8StringEncoding],NO);}
  if(!raw)return SaveAction(2,paths.firstObject,NO);
  NSData *data=[NSJSONSerialization dataWithJSONObject:@{@"paths":paths,@"replace_matching":@(replace)} options:0 error:NULL];return SaveAction(8,[[NSString alloc]initWithData:data encoding:NSUTF8StringEncoding],NO);
 });} completion:^(NSDictionary *result){
  if(progress||merge){[self presentProgressPreview:result];return;}
  if(raw){[self presentRawPreview:result];return;}
  NSString *message=[NSString stringWithFormat:@"Validated %@ in-game user(s) and %@ save files for %@.\n\nRestore all save files in this backup? Your current save files will be retained so you can restore them later. Game Center data and app settings are not included in the backup.",result[@"profiles"],result[@"files"],result[@"game"]];
  
  UIAlertController *alert=[UIAlertController alertControllerWithTitle:@"Restore Backup?" message:message preferredStyle:UIAlertControllerStyleAlert];
  [alert addAction:[UIAlertAction actionWithTitle:@"Cancel" style:UIAlertActionStyleCancel handler:^(UIAlertAction *action){[self run:7 value:@"" confirmed:NO completion:^(NSDictionary *done){[self message:@"Your save files are unchanged." title:@"Import Cancelled"];}];}]];
  [alert addAction:[UIAlertAction actionWithTitle:@"Restore Backup" style:UIAlertActionStyleDestructive handler:^(UIAlertAction *action){[self run:3 value:result[@"token"] confirmed:YES completion:^(NSDictionary *done){[self message:@"Your backup was restored. The previous set of save files is available in Export." title:@"Import Complete"];}];}]];
  [self presentPanel:alert];
 }];
}
- (void)documentPickerWasCancelled:(UIDocumentPickerViewController *)controller {
 BOOL exporting=[controller isKindOfClass:PeggleSavePicker.class]&&[(PeggleSavePicker *)controller exportSelection];[self message:exporting?@"No backup was saved to Files.":@"Your save files are unchanged." title:exporting?@"Export Cancelled":@"Import Cancelled"];
}
- (void)rollback {
 UIAlertController *alert=[UIAlertController alertControllerWithTitle:@"Restore Previous Save Files?" message:@"Your current set will also be retained so you can switch back. The game has not started." preferredStyle:UIAlertControllerStyleAlert];
 [alert addAction:[UIAlertAction actionWithTitle:@"Cancel" style:UIAlertActionStyleCancel handler:nil]];
 [alert addAction:[UIAlertAction actionWithTitle:@"Restore Previous" style:UIAlertActionStyleDestructive handler:^(UIAlertAction *action){[self run:4 value:@"" confirmed:YES completion:^(NSDictionary *result){[self message:@"The previous set of save files was restored. You can switch back using the same control." title:@"Save Files Restored"];}];}]];
 [self presentPanel:alert];
}
@end

// SDL_main already runs inside UIKit's event loop. Keep the interpreter stopped
// while a nested native run loop services this pre-launch controller, then leave
// the manager window before the unchanged SDL/keyboard/game startup proceeds.
int peggle_present_save_manager(NSString *documents){
 if(!NSThread.isMainThread)return 0;
 UIWindowScene *scene=nil;for(UIScene *candidate in UIApplication.sharedApplication.connectedScenes){if([candidate isKindOfClass:UIWindowScene.class]&&(candidate.activationState==UISceneActivationStateForegroundActive||candidate.activationState==UISceneActivationStateForegroundInactive)){scene=(UIWindowScene *)candidate;break;}}
 if(!scene)return 0;
 NSUserDefaults *defaults=NSUserDefaults.standardUserDefaults;
 [defaults registerDefaults:@{@"PeggleShowSaveManagementOnLaunch":@YES}];
 NSDictionary *opened=SaveAction(0,documents,NO);
 if([opened[@"ok"]boolValue])PeggleAutoPrepare(documents,NO);
 // Even without the UI, resolve the active save slot and acquire the game lease.
 // If either operation fails, show the manager's error rather than starting unsafely.
 if(![defaults boolForKey:@"PeggleShowSaveManagementOnLaunch"]&&[opened[@"ok"]boolValue]&&!PeggleAutoStartupIssue()){
  NSDictionary *started=SaveAction(5,@"",NO);
  if([started[@"ok"]boolValue])return 1;
  NSMutableDictionary *failure=[opened mutableCopy];failure[@"error"]=started[@"error"]?:@"Could not open the selected save files.";opened=failure;
 }
 PeggleSaveManagerController *controller=[PeggleSaveManagerController new];controller.initialized=[opened[@"ok"]boolValue];controller.canRollback=[opened[@"can_rollback"]boolValue];controller.initialMessage=opened[@"error"]?:PeggleAutoStartupIssue();
 UIWindow *window=[[UIWindow alloc]initWithWindowScene:scene];window.tintColor=[UIColor colorWithRed:246.0/255 green:117.0/255 blue:39.0/255 alpha:1];window.windowLevel=UIWindowLevelNormal+1;window.rootViewController=controller;[window makeKeyAndVisible];
 while(!controller.ready){@autoreleasepool{[NSRunLoop.mainRunLoop runMode:NSDefaultRunLoopMode beforeDate:[NSDate dateWithTimeIntervalSinceNow:.05]];}}
 window.hidden=YES;window.rootViewController=nil;return 1;
}
