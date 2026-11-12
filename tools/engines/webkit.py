# WebKitGTK 4.1 (o motor do Tauri no Linux) abrindo a URL em tela cheia
import sys, gi
gi.require_version('Gtk', '3.0'); gi.require_version('WebKit2', '4.1')
from gi.repository import Gtk, WebKit2
w = Gtk.Window(); w.set_default_size(1920, 1080)
v = WebKit2.WebView(); v.load_uri(sys.argv[1]); w.add(v); w.connect('destroy', Gtk.main_quit)
w.show_all(); w.fullscreen(); Gtk.main()
