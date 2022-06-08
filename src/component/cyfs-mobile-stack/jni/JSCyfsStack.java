package com.cyfs;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.net.ConnectivityManager;
import android.net.LinkAddress;
import android.net.LinkProperties;
import android.net.Network;
import android.net.NetworkCapabilities;
import android.os.Build;
import android.util.Log;

import androidx.annotation.NonNull;
import androidx.annotation.Nullable;

import com.facebook.react.bridge.Arguments;
import com.facebook.react.bridge.ReactApplicationContext;
import com.facebook.react.bridge.ReactContextBaseJavaModule;
import com.facebook.react.bridge.ReactMethod;
import com.facebook.react.bridge.ReadableMap;
import com.facebook.react.bridge.WritableMap;
import com.facebook.react.modules.core.DeviceEventManagerModule;

import java.io.IOException;
import java.net.HttpURLConnection;
import java.net.Inet4Address;
import java.net.InetAddress;
import java.net.URL;
import java.util.ArrayDeque;
import java.util.List;
import java.util.Queue;
import java.util.Timer;
import java.util.TimerTask;
public class JSCyfsStack extends ReactContextBaseJavaModule {
    private class CCNetworkListener extends ConnectivityManager.NetworkCallback {
        private class ResetStackTimerTask extends TimerTask {
            private CCNetworkListener owner;
            public ResetStackTimerTask(CCNetworkListener owner) {
                super();
                this.owner = owner;
            }
            @Override
            public void run() {
                this.owner.timing = false;
                Stack.resetNetwork(this.owner.addr);
            }
        }
        private boolean first = true;
        private boolean timing = false;
        private String addr;
        public void onLinkPropertiesChanged(Network network, LinkProperties linkProperties) {
            if (this.first) {
                this.first = false;
                return;
            }
            String addr = getWifiIpv4Addr();

            String log = String.format("link changed, interface %s, ipv4 addrs %s", linkProperties.getInterfaceName(), addr);
            Log.i("cyfsstack", log);
            this.addr = addr;
            if (!this.timing) {
                this.timing = true;

                Timer timer = new Timer();
                // 延迟3秒启动, 这3秒间的通知只会应用最后一个
                timer.schedule(new ResetStackTimerTask(this), 3000);
            }
        }
    }

    private static JSCyfsStack instance = null;
    private ReactApplicationContext context;
    private CCNetworkListener network_listener;
    private String client_addr;


    public static JSCyfsStack getInstance(ReactApplicationContext reactContext){
        if(null == instance && reactContext != null){

            instance = new JSCyfsStack(reactContext);
        }
        return instance;
    }

    public static boolean isInit() {
        return instance != null;
    }

    private JSCyfsStack(ReactApplicationContext reactContext) {
        super(reactContext);
        this.context = reactContext;
        this.network_listener = new CCNetworkListener();
    }

    @NonNull
    @Override
    public String getName() {
        return "CyfsStack";
    }

    public String getWifiIpv4Addr() {
        ConnectivityManager cm = (ConnectivityManager) getReactApplicationContext().getApplicationContext().getSystemService(Context.CONNECTIVITY_SERVICE);
        Network net = cm.getActiveNetwork();
        if (net == null) {return "";}
        NetworkCapabilities nc = cm.getNetworkCapabilities(net);
        if (nc == null) {return "";}
        LinkProperties lp = cm.getLinkProperties(net);
        // 无论是什么网络，都返回找到的ipv4地址
        List<LinkAddress> linkAddrs = lp.getLinkAddresses();

        for (int i = 0; i < linkAddrs.size(); i++) {
            InetAddress inet_addr = linkAddrs.get(i).getAddress();
            if (inet_addr instanceof Inet4Address) {
                return inet_addr.toString().substring(1);
            }
        }
        return "";
    }

    @ReactMethod
    public void restartInterface() {
        Stack.restartInterface();
    }

    @ReactMethod
    public void start(ReadableMap params) {
        String base_path = params.getString("base_path");
        String non_addr = params.getString("non_addr");
        String ws_addr = params.getString("ws_addr");
        int bdt_port = params.getInt("bdt_port");
        String loglevel = params.getString("loglevel");

        // 尝试取wifi的ipv4地址
        String wifi_ipv4_addr = getWifiIpv4Addr();

        Stack.client_start(base_path, non_addr, ws_addr, bdt_port, loglevel, wifi_ipv4_addr);

        // 监听网络Link改变事件，先只写一个Android7以上有效的方法
        ConnectivityManager cm = (ConnectivityManager) getReactApplicationContext().getApplicationContext().getSystemService(Context.CONNECTIVITY_SERVICE);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            cm.registerDefaultNetworkCallback(this.network_listener);
        } else {
            Log.e("CyfsStack", "unsupport Android < 7");
        }
    }
}