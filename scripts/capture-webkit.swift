import AppKit
import Foundation
import WebKit

guard CommandLine.arguments.count == 5,
      let width = Double(CommandLine.arguments[2]),
      let height = Double(CommandLine.arguments[3]) else {
    fputs("usage: capture-webkit.swift <url> <width> <height> <output.png>\n", stderr)
    exit(2)
}

let targetURL = URL(string: CommandLine.arguments[1])!
let outputURL = URL(fileURLWithPath: CommandLine.arguments[4])
let app = NSApplication.shared

final class CaptureDelegate: NSObject, WKNavigationDelegate {
    let webView: WKWebView
    let outputURL: URL

    init(webView: WKWebView, outputURL: URL) {
        self.webView = webView
        self.outputURL = outputURL
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) { [self] in
            capture()
        }
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        fputs("navigation failed: \(error)\n", stderr)
        exit(1)
    }

    private func capture() {
        let config = WKSnapshotConfiguration()
        config.rect = webView.bounds
        webView.takeSnapshot(with: config) { image, error in
            if let error {
                fputs("snapshot failed: \(error)\n", stderr)
                exit(1)
            }
            guard let image,
                  let tiff = image.tiffRepresentation,
                  let bitmap = NSBitmapImageRep(data: tiff),
                  let png = bitmap.representation(using: .png, properties: [:]) else {
                fputs("failed to encode PNG\n", stderr)
                exit(1)
            }
            do {
                try png.write(to: self.outputURL)
                print(self.outputURL.path)
                NSApp.terminate(nil)
            } catch {
                fputs("failed to write PNG: \(error)\n", stderr)
                exit(1)
            }
        }
    }
}

let configuration = WKWebViewConfiguration()
configuration.websiteDataStore = .nonPersistent()
let webView = WKWebView(
    frame: NSRect(x: 0, y: 0, width: width, height: height),
    configuration: configuration
)
let delegate = CaptureDelegate(webView: webView, outputURL: outputURL)
webView.navigationDelegate = delegate

let window = NSWindow(
    contentRect: webView.frame,
    styleMask: [.borderless],
    backing: .buffered,
    defer: false
)
window.contentView = webView
window.orderFrontRegardless()
webView.load(URLRequest(url: targetURL))
app.run()
