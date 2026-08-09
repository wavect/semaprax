#import <Cocoa/Cocoa.h>
#import <CommonCrypto/CommonDigest.h>
#include <signal.h>

static NSString *const kButtonName = @"Run SEMAPRAX verification";
static NSString *const kEngineManifestPrefix =
    @"semaprax.private-desktop-engine-sha256.v1 ";
static const NSTimeInterval kEngineDeadlineSeconds = 5.0;
static const NSTimeInterval kEngineTerminationGraceSeconds = 1.0;
static const NSTimeInterval kEngineKillGraceSeconds = 1.0;
static NSString *const kEngineOutput =
    @"SEMAPRAX_DESKTOP_V3_OK platform=macos calls=2 owner=0 "
     "payloads=41,43 replay=exact\n";
static NSString *const kUiOutput =
    @"SEMAPRAX_DESKTOP_UI_V1_OK platform=macos "
     "lifecycle=launch,window,shown,control,close,terminate "
     "accessibility=button-name engine=calls-2-replay-exact\n";

@interface SemapraxUiController
    : NSObject <NSApplicationDelegate, NSWindowDelegate>
@property(nonatomic, strong) NSWindow *window;
@property(nonatomic, strong) NSButton *button;
@property(nonatomic, copy) NSString *resultPath;
@property(nonatomic) BOOL launched;
@property(nonatomic) BOOL windowCreated;
@property(nonatomic) BOOL shown;
@property(nonatomic) BOOL controlled;
@property(nonatomic) BOOL closed;
@property(nonatomic) BOOL enginePassed;
@property(nonatomic) int exitCode;
@end

@implementation SemapraxUiController

- (void)fail:(NSString *)message {
  if (self.exitCode == 0) {
    self.exitCode = 1;
    fprintf(stderr, "SEMAPRAX desktop UI failure: %s\n",
            message.UTF8String);
  }
  // `terminate:` exits the process with status zero after notifying the
  // delegate, which would make a rejected hostile engine indistinguishable
  // from a successful application run. Stop the event loop so `main` returns
  // the controller's stable nonzero status instead.
  [NSApp stop:nil];
}

- (BOOL)runEngine {
  NSString *engine = [[NSBundle mainBundle].bundlePath
      stringByAppendingPathComponent:
          @"Contents/Resources/SemapraxPrivateEngine"];
  if (![[NSFileManager defaultManager] isExecutableFileAtPath:engine]) {
    [self fail:@"packaged engine is missing or not executable"];
    return NO;
  }
  NSString *manifest = [[NSBundle mainBundle].bundlePath
      stringByAppendingPathComponent:
          @"Contents/Resources/SemapraxPrivateEngine.sha256"];
  NSError *readError = nil;
  NSString *manifestText = [NSString stringWithContentsOfFile:manifest
                                                      encoding:NSASCIIStringEncoding
                                                         error:&readError];
  if (manifestText == nil ||
      manifestText.length != kEngineManifestPrefix.length + 65 ||
      ![manifestText hasPrefix:kEngineManifestPrefix] ||
      ![manifestText hasSuffix:@"\n"]) {
    [self fail:@"engine digest manifest is missing or malformed"];
    return NO;
  }
  NSString *expectedHex =
      [manifestText substringWithRange:
                        NSMakeRange(kEngineManifestPrefix.length, 64)];
  NSCharacterSet *lowerHex =
      [NSCharacterSet characterSetWithCharactersInString:@"0123456789abcdef"];
  if ([expectedHex rangeOfCharacterFromSet:lowerHex.invertedSet].location !=
      NSNotFound) {
    [self fail:@"engine digest manifest is not canonical lowercase hex"];
    return NO;
  }
  NSData *engineBytes = [NSData dataWithContentsOfFile:engine
                                               options:NSDataReadingMappedIfSafe
                                                 error:&readError];
  if (engineBytes == nil || engineBytes.length > UINT32_MAX) {
    [self fail:@"engine bytes could not be bounded for digest verification"];
    return NO;
  }
  unsigned char digest[CC_SHA256_DIGEST_LENGTH];
  if (CC_SHA256(engineBytes.bytes, (CC_LONG)engineBytes.length, digest) ==
      NULL) {
    [self fail:@"engine digest computation failed"];
    return NO;
  }
  static const char hexDigits[] = "0123456789abcdef";
  char actualHex[(CC_SHA256_DIGEST_LENGTH * 2) + 1];
  for (NSUInteger index = 0; index < CC_SHA256_DIGEST_LENGTH; ++index) {
    actualHex[index * 2] = hexDigits[digest[index] >> 4];
    actualHex[(index * 2) + 1] = hexDigits[digest[index] & 0x0f];
  }
  actualHex[CC_SHA256_DIGEST_LENGTH * 2] = '\0';
  NSString *actualDigest = [NSString stringWithUTF8String:actualHex];
  if (actualDigest == nil || ![actualDigest isEqualToString:expectedHex]) {
    [self fail:@"engine bytes do not match the packaged digest manifest"];
    return NO;
  }

  NSPipe *output = [NSPipe pipe];
  NSTask *task = [[NSTask alloc] init];
  task.executableURL = [NSURL fileURLWithPath:engine];
  task.standardOutput = output;
  task.standardError = [NSPipe pipe];
  NSError *launchError = nil;
  if (![task launchAndReturnError:&launchError]) {
    [self fail:[NSString stringWithFormat:@"engine launch failed: %@",
                                          launchError.localizedDescription]];
    return NO;
  }
  NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:
                                 kEngineDeadlineSeconds];
  while (task.running && deadline.timeIntervalSinceNow > 0) {
    (void)[[NSRunLoop currentRunLoop]
        runMode:NSDefaultRunLoopMode
     beforeDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
  }
  if (task.running) {
    [task terminate];
    NSDate *terminationDeadline = [NSDate
        dateWithTimeIntervalSinceNow:kEngineTerminationGraceSeconds];
    while (task.running && terminationDeadline.timeIntervalSinceNow > 0) {
      (void)[[NSRunLoop currentRunLoop]
          runMode:NSDefaultRunLoopMode
       beforeDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
    }
    if (task.running) {
      if (kill(task.processIdentifier, SIGKILL) != 0) {
        [self fail:@"engine timeout could not deliver SIGKILL"];
        return NO;
      }
      NSDate *killDeadline =
          [NSDate dateWithTimeIntervalSinceNow:kEngineKillGraceSeconds];
      while (task.running && killDeadline.timeIntervalSinceNow > 0) {
        (void)[[NSRunLoop currentRunLoop]
            runMode:NSDefaultRunLoopMode
         beforeDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
      }
      if (task.running) {
        [self fail:@"engine remained live after bounded SIGKILL grace"];
        return NO;
      }
    }
    [self fail:@"engine exceeded its bounded execution deadline"];
    return NO;
  }
  NSData *bytes = [output.fileHandleForReading readDataToEndOfFile];
  NSString *actual = [[NSString alloc] initWithData:bytes
                                           encoding:NSUTF8StringEncoding];
  if (task.terminationStatus != 0 || ![actual isEqualToString:kEngineOutput]) {
    [self fail:@"engine output or exit status changed"];
    return NO;
  }
  self.enginePassed = YES;
  return YES;
}

- (void)applicationDidFinishLaunching:(NSNotification *)notification {
  (void)notification;
  self.launched = YES;

  NSRect frame = NSMakeRect(0, 0, 480, 220);
  self.window = [[NSWindow alloc]
      initWithContentRect:frame
                styleMask:(NSWindowStyleMaskTitled |
                           NSWindowStyleMaskClosable)
                  backing:NSBackingStoreBuffered
                    defer:NO];
  self.window.title = @"SEMAPRAX Native Verification";
  self.window.delegate = self;
  self.window.releasedWhenClosed = NO;
  self.windowCreated = YES;

  NSTextField *summary = [NSTextField
      labelWithString:@"Meaning in. Verified machine code out."];
  summary.frame = NSMakeRect(70, 145, 340, 24);
  summary.alignment = NSTextAlignmentCenter;
  [self.window.contentView addSubview:summary];

  self.button = [NSButton buttonWithTitle:kButtonName
                                   target:self
                                   action:@selector(controlInvoked:)];
  self.button.frame = NSMakeRect(120, 72, 240, 42);
  self.button.bezelStyle = NSBezelStyleRounded;
  self.button.accessibilityLabel = kButtonName;
  self.button.accessibilityElement = YES;
  [self.window.contentView addSubview:self.button];

  [self.window center];
  [self.window makeKeyAndOrderFront:nil];
  [NSApp activateIgnoringOtherApps:YES];

  dispatch_after(
      dispatch_time(DISPATCH_TIME_NOW, (int64_t)(100 * NSEC_PER_MSEC)),
      dispatch_get_main_queue(), ^{
        if (!self.window.visible || self.button.hidden) {
          [self fail:@"native window or control did not become visible"];
          return;
        }
        if (!self.button.accessibilityElement ||
            ![self.button.accessibilityLabel isEqualToString:kButtonName]) {
          [self fail:@"native control accessibility name changed"];
          return;
        }
        self.shown = YES;
        if ([self runEngine]) {
          [self.button performClick:nil];
        }
      });
}

- (void)controlInvoked:(id)sender {
  if (sender != self.button || !self.shown || !self.enginePassed) {
    [self fail:@"native control event arrived out of lifecycle order"];
    return;
  }
  self.controlled = YES;
  [self.window performClose:nil];
}

- (void)windowWillClose:(NSNotification *)notification {
  if (notification.object != self.window || !self.controlled) {
    [self fail:@"native close event arrived out of lifecycle order"];
    return;
  }
  self.closed = YES;
  [NSApp terminate:nil];
}

- (void)applicationWillTerminate:(NSNotification *)notification {
  (void)notification;
  if (self.exitCode != 0) {
    return;
  }
  if (!self.launched || !self.windowCreated || !self.shown ||
      !self.controlled || !self.closed || !self.enginePassed) {
    self.exitCode = 1;
    fprintf(stderr, "SEMAPRAX desktop UI lifecycle was incomplete\n");
    return;
  }
  NSError *writeError = nil;
  if (![kUiOutput writeToFile:self.resultPath
                   atomically:YES
                     encoding:NSUTF8StringEncoding
                        error:&writeError]) {
    self.exitCode = 1;
    fprintf(stderr, "SEMAPRAX desktop UI result write failed: %s\n",
            writeError.localizedDescription.UTF8String);
  }
}

@end

int main(int argc, const char *argv[]) {
  @autoreleasepool {
    if (argc != 2) {
      fprintf(stderr,
              "usage: SemapraxPrivate ABSOLUTE_NEW_RESULT_FILE\n");
      return 2;
    }
    NSString *resultPath = [NSString stringWithUTF8String:argv[1]];
    if (resultPath == nil || !resultPath.isAbsolutePath ||
        [[NSFileManager defaultManager] fileExistsAtPath:resultPath]) {
      fprintf(stderr, "result file must be a new absolute path\n");
      return 2;
    }

    NSApplication *application = [NSApplication sharedApplication];
    application.activationPolicy = NSApplicationActivationPolicyRegular;
    SemapraxUiController *controller = [[SemapraxUiController alloc] init];
    controller.resultPath = resultPath;
    controller.exitCode = 0;
    application.delegate = controller;
    [application run];
    return controller.exitCode;
  }
}
