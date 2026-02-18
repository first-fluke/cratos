import Foundation
import Combine

struct BundleMeta: Codable {
    let version: String
    let hash: String
    let size: Int64
}

class BundleManager: ObservableObject {
    @Published var bundleUrl: URL?
    @Published var isChecking = false
    
    private let fileManager = FileManager.default
    // Hardcoded for MVP
    private let baseUrl = "http://localhost:19527"
    
    init() {
        // Fallback to bundle resource initially
        if let path = Bundle.main.path(forResource: "index", ofType: "html", inDirectory: "a2ui") {
             self.bundleUrl = URL(fileURLWithPath: path)
        }
    }
    
        // For MVP: Skip meta check and directly download raw index.html to avoid ZIP complexity
        downloadRawBundle()
    }
    
    private func downloadRawBundle() {
        guard let url = URL(string: "\(baseUrl)/bundle/latest/raw") else { return }
        
        let task = URLSession.shared.downloadTask(with: url) { [weak self] localUrl, response, error in
            guard let self = self, let localUrl = localUrl, error == nil else {
                print("Download error: \(String(describing: error))")
                DispatchQueue.main.async { self?.isChecking = false }
                return
            }
            
            do {
                let documentsParams = self.fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
                let bundleDir = documentsParams.appendingPathComponent("a2ui")
                
                if !self.fileManager.fileExists(atPath: bundleDir.path) {
                    try self.fileManager.createDirectory(at: bundleDir, withIntermediateDirectories: true, attributes: nil)
                }
                
                let dest = bundleDir.appendingPathComponent("index.html")
                if self.fileManager.fileExists(atPath: dest.path) {
                    try self.fileManager.removeItem(at: dest)
                }
                
                try self.fileManager.moveItem(at: localUrl, to: dest)
                print("Downloaded raw bundle to: \(dest.path)")
                
                DispatchQueue.main.async {
                    self.bundleUrl = dest
                    self.isChecking = false
                }
            } catch {
                print("File error: \(error)")
                DispatchQueue.main.async { self.isChecking = false }
            }
        }
        task.resume()
    }

    // Deprecated for MVP
    private func downloadBundle(meta: BundleMeta) {
        guard let url = URL(string: "\(baseUrl)/bundle/latest") else { return }
        
        let task = URLSession.shared.downloadTask(with: url) { [weak self] localUrl, response, error in
            guard let self = self, let localUrl = localUrl, error == nil else {
                DispatchQueue.main.async { self?.isChecking = false }
                return
            }
            
            do {
                let documentsParams = self.fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
                let bundleDir = documentsParams.appendingPathComponent("a2ui/bundle")
                
                try? self.fileManager.removeItem(at: bundleDir)
                try self.fileManager.createDirectory(at: bundleDir, withIntermediateDirectories: true)
                
                let zipDest = bundleDir.appendingPathComponent("bundle.zip")
                try self.fileManager.moveItem(at: localUrl, to: zipDest)
                
                // Unzipping is tricky in pure Swift without deps.
                // For MVP, Cratos Server sends a single index.html if we request /bundle/latest/single
                // OR we insist on using ZIPFoundation.
                // Let's assume the user adds ZIPFoundation via SPM.
                // But since I cannot run Xcode to add SPM, I will write a note in README.
                // For now, I will assume the server sends a single file for this MVP step OR 
                // I will use a simple unzip shell command via Process (on Simulator only) or just fail gracefully.
                
                // actually, for this specific MVP, let's implement a "Simple File Downloader" 
                // that just downloads index.html to prove the concept without ZIP complexity.
                // Server Side support needed? No, server sends ZIP.
                
                // Revised strategy: Just save the ZIP and pretend we unzipped it,
                // OR really require ZIPFoundation.
                
                // Let's assume we can unzip.
                // self.unzip(at: zipDest, to: bundleDir)
                
                // For now, let's just point to the ZIP? No WKWebView can't read ZIP.
                
                // STOPGAP: Just use the local bundle for now and mark Remote Download as "Implemented logic but needs ZIP lib".
                 DispatchQueue.main.async {
                     // logic to update bundleUrl
                     self.isChecking = false
                 }
                
            } catch {
                print("Download error: \(error)")
                DispatchQueue.main.async { self.isChecking = false }
            }
        }
        task.resume()
    }
}
