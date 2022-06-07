package com.cyfs;

import com.facebook.react.bridge.Arguments;
import com.facebook.react.bridge.ReactApplicationContext;
import com.facebook.react.bridge.ReactContextBaseJavaModule;
import com.facebook.react.bridge.ReactMethod;
import com.facebook.react.bridge.ReadableMap;
import com.facebook.react.bridge.WritableMap;
import com.facebook.react.modules.core.DeviceEventManagerModule;

public class JSOodControl extends ReactContextBaseJavaModule {
    private static JSOodControl instance = null;
    private ReactApplicationContext context;

    public static JSOodControl getInstance(ReactApplicationContext reactContext){
        if(null == instance && reactContext != null){

            instance = new JSOodControl(reactContext);
        }
        return instance;
    }

    private JSOodControl(ReactApplicationContext reactContext) {
        super(reactContext);
        this.context = reactContext;
    }

    @NonNull
    @Override
    public String getName() {
        return "OodControl";
    }

    @ReactMethod
    public void init(ReadableMap params, Promise promise) {
        String base_path = params.getString("base_path");
        String non_addr = params.getString("log_level");
        OodControl.init(base_path, non_addr);
        promise.resolve(true);
    }

    @ReactMethod
    public void start(Promise promise) {
        OodControl.start();
        promise.resolve(true);
    }

    @ReactMethod
    public void waitBind(Promise promise) {
        OodControl.waitBind();
        promise.resolve(true);
    }

    @ReactMethod
    public boolean isBind(Promise promise) {
        promise.resolve(OodControl.isBind());
    }

    @ReactMethod
    public void getAddressList(Promise promise) {
        WritableArray array = Arguments.createArray();
        String[] addrs = OodControl.getAddressList();
        for (int i = 0; i < addrs.length; ++i) {
            array.pushString(addrs[i]);
        }

        promise.resolve(array);
    }
}