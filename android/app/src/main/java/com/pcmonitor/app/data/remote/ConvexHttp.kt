package com.pcmonitor.app.data.remote

import android.util.Log
import okhttp3.Dns
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject
import java.net.InetAddress

/** Shared pieces of the calls the app makes to Convex's HTTP API. */
object ConvexHttp {
    const val CLOUD_URL = "https://tidy-newt-65.convex.cloud"

    private const val TAG = "ConvexHttp"

    /** Falls back to fixed addresses when the system resolver cannot find the Convex host. */
    val fallbackDns: Dns = object : Dns {
        override fun lookup(hostname: String): List<InetAddress> {
            return try {
                Dns.SYSTEM.lookup(hostname)
            } catch (e: Exception) {
                if (hostname.contains("convex.cloud")) {
                    Log.i(TAG, "Using fallback IP for $hostname")
                    listOf(
                        InetAddress.getByAddress(hostname, byteArrayOf(104.toByte(), 18.toByte(), 14.toByte(), 131.toByte())),
                        InetAddress.getByAddress(hostname, byteArrayOf(104.toByte(), 18.toByte(), 15.toByte(), 131.toByte()))
                    )
                } else {
                    throw e
                }
            }
        }
    }

    /** POST `{"path": path, "args": args}` to `$baseUrl/api/$kind` ("query", "mutation" or "action"). */
    fun request(kind: String, path: String, args: JSONObject, baseUrl: String = CLOUD_URL): Request {
        val body = JSONObject().put("path", path).put("args", args)
        return Request.Builder()
            .url("$baseUrl/api/$kind")
            .post(body.toString().toRequestBody("application/json; charset=utf-8".toMediaType()))
            .build()
    }
}
