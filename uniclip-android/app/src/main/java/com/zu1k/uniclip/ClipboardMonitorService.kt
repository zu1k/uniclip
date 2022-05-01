package com.zu1k.uniclip

import android.app.Service
import android.content.ClipData
import android.content.Intent
import android.os.IBinder
import android.content.ClipboardManager
import android.content.Context
import java.lang.Exception

class ClipboardMonitorService(): Service() {
    private lateinit var clipboard: ClipboardManager
    private var lastText = "";

    constructor(context: Context) : this() {
        clipboard = context!!.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    }

    fun copyToClipboard(text: String) {
        println("copyToClipboard: $text")
        return try {
            val clip = ClipData.newPlainText("UniClip", text)
            clipboard.setPrimaryClip(clip)
            lastText = text
        } catch (e: Exception) {
        }
    }

    fun readFromClipboard(): String {
        val clip = clipboard.primaryClip
        if (clip != null) {
            val item = clip.getItemAt(0)
            val text = item.text
            if (text != null) {
                return text.toString()
            }
            val uri = item.uri
            if (uri != null) {
                return uri.toString()
            }
            val intent = item.intent
            return if (intent != null) {
                intent.toUri(Intent.URI_INTENT_SCHEME)
            } else ""
        }
        return ""
    }

    override fun onCreate() {
        super.onCreate()
        clipboard = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        clipboard.addPrimaryClipChangedListener(mOnPrimaryClipChangedListener)
    }

    override fun onDestroy() {
        super.onDestroy()
        clipboard.removePrimaryClipChangedListener(mOnPrimaryClipChangedListener)
    }

    override fun onBind(intent: Intent?): IBinder? {
        return null
    }

    fun listenClipboard() {
        val text = readFromClipboard()
        println("clipboard changed: $text")
        if (text != lastText) {
            lastText = text
            clipPublishText(text)
            println("down")
        }
    }

    private val mOnPrimaryClipChangedListener =
        ClipboardManager.OnPrimaryClipChangedListener {
            listenClipboard()
        }

    external fun clipPublishText(text: String)

    companion object {
        init {
            System.loadLibrary("uniclip")
        }
    }

}