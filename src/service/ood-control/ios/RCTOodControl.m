#import <Foundation/Foundation.h>
#import "RCTOodControl.h.h"
#import "oodcontrol.h"

@implementation RCTOodControl

RCT_EXPORT_MODULE(OodControl)

static void log_c(const uint8_t* msg) {
    NSLog(@"%s", msg);
}

RCT_EXPORT_BLOCKING_SYNCHRONOUS_METHOD(init:(NSDictionary*)params)
{
    NSString* base_path = [params valueForKey:@"base_path"];
    NSString* loglevel = [params valueForKey:@"loglevel"];

    init([base_path UTF8String], [loglevel UTF8String], (void (*)(const uint8_t *))log_c);
}

RCT_EXPORT_BLOCKING_SYNCHRONOUS_METHOD(waitBind)
{
    dispatch_queue_t queue = dispatch_queue_create("rust_cocurrent_queue", DISPATCH_QUEUE_CONCURRENT);

    dispatch_async(queue, ^{
        NSLog(@"wait bind in new thread.");
        wait_bind();
    });
}

RCT_EXPORT_BLOCKING_SYNCHRONOUS_METHOD(isBind)
{
    return is_bind();
}

RCT_EXPORT_BLOCKING_SYNCHRONOUS_METHOD(start)
{
    start();
}

RCT_EXPORT_BLOCKING_SYNCHRONOUS_METHOD(getAddressList) {
    Result* result = get_address_list();
    NSMutableArray *mutArr = [[ NSMutableArray alloc]initWithCapacity:result->result_num];
    for (int i = 0; i < result->result_num; i++) {
        NSString* value = [[NSString alloc] initWithUTF8String:result->result_value[i]];
        [array addObject:value];
    }
    free_result(result);
    return mutArr;
}

