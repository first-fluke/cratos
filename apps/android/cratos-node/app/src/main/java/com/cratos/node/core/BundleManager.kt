package com.cratos.node.core

import android.content.Context
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.util.zip.ZipInputStream
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

@Serializable
data class BundleMeta(
    val version: String,
    val hash: String,
    val size: Long
)

@Singleton
class BundleManager @Inject constructor(
    @ApplicationContext private val context: Context
) {
    private val client = OkHttpClient()
    private val json = Json { ignoreUnknownKeys = true }
    // Hardcoded for MVP
    private val baseUrl = "http://10.0.2.2:19527" 

    suspend fun getLocalBundleEntry(entryPoint: String = "index.html"): String? = withContext(Dispatchers.IO) {
        val bundleDir = File(context.filesDir, "a2ui/bundle")
        val file = File(bundleDir, entryPoint)
        if (file.exists()) "file://${file.absolutePath}" else null
    }

    suspend fun checkForUpdates(): Boolean = withContext(Dispatchers.IO) {
        try {
            // 1. Get Meta
            val request = Request.Builder().url("$baseUrl/bundle/meta").build()
            val response = client.newCall(request).execute()
            
            if (!response.isSuccessful) return@withContext false
            
            val metaString = response.body?.string() ?: return@withContext false
            val meta = json.decodeFromString<BundleMeta>(metaString)

            // 2. Check Local (Simple check for now, can be improved with preferences)
            val bundleDir = File(context.filesDir, "a2ui/bundle")
            if (!bundleDir.exists()) {
                downloadAndUnzip()
            } else {
                 // TODO: Compare hash
                 // For MVP, always redownload if requested or implement proper hash check
                 // Here we just return true if exists, or download if not.
                 // To force update, we can check a stored preference.
                 true
            }
        } catch (e: Exception) {
            e.printStackTrace()
            false
        }
    }

    private fun downloadAndUnzip(): Boolean {
        try {
            val request = Request.Builder().url("$baseUrl/bundle/latest").build()
            val response = client.newCall(request).execute()
            
            if (!response.isSuccessful) return false
            
            val byteStream = response.body?.byteStream() ?: return false
            val bundleDir = File(context.filesDir, "a2ui/bundle")
            
            if (bundleDir.exists()) bundleDir.deleteRecursively()
            bundleDir.mkdirs()

            ZipInputStream(byteStream).use { zip ->
                var entry = zip.nextEntry
                while (entry != null) {
                    val file = File(bundleDir, entry.name)
                    if (entry.isDirectory) {
                        file.mkdirs()
                    } else {
                        file.parentFile?.mkdirs()
                        file.outputStream().use { output ->
                            zip.copyTo(output)
                        }
                    }
                    entry = zip.nextEntry
                }
            }
            return true
        } catch (e: Exception) {
            e.printStackTrace()
            return false
        }
    }
}
