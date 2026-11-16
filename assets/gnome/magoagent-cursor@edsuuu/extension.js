// MagoAgent: where the pointer is anywhere on the screen, for the island's eyes. The app runs on XWayland, whose X
// server only knows the pointer over its windows; it asks here over D-Bus (src/app/island/gnome.rs).
import Gio from 'gi://Gio';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const PATH = '/io/github/edsuuu/MagoAgent/Cursor';
const IFACE = `<node>
  <interface name="io.github.edsuuu.MagoAgent.Cursor">
    <method name="Where">
      <arg type="i" direction="out" name="x"/>
      <arg type="i" direction="out" name="y"/>
    </method>
  </interface>
</node>`;

export default class MagoAgentCursor extends Extension {
    enable() {
        this._object = Gio.DBusExportedObject.wrapJSObject(IFACE, this);
        this._object.export(Gio.DBus.session, PATH);
    }

    disable() {
        this._object?.unexport();
        this._object = null;
    }

    Where() {
        const [x, y] = global.get_pointer();
        return [x, y];
    }
}
