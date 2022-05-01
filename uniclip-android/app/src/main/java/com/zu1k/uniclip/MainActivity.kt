package com.zu1k.uniclip

import androidx.appcompat.app.AppCompatActivity
import android.os.Bundle
import android.content.Intent
import com.zu1k.uniclip.databinding.ActivityMainBinding
import kotlin.concurrent.thread

class MainActivity : AppCompatActivity() {
    private lateinit var clipboard: ClipboardMonitorService;

    private lateinit var binding: ActivityMainBinding
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        binding = ActivityMainBinding.inflate(layoutInflater)
        setContentView(binding.root)

        binding.sampleText.text = stringFromJNI()
        binding.root.setOnClickListener {
            clipboard.listenClipboard()
        }

        clipboard =  ClipboardMonitorService(baseContext);
        startService(Intent(this, ClipboardMonitorService::class.java))

        thread(start = true) {
            println("running from thread(): ${Thread.currentThread()}")
            start("uniclip-android", clipboard)
        }
    }

    external fun stringFromJNI(): String
    external fun start(topic: String, callback: ClipboardMonitorService)

    companion object {
        init {
            System.loadLibrary("uniclip")
        }
    }
}