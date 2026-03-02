import Foundation
import CStdLib

@_cdecl("swift_exported_function")
func swiftExportedFunction(_ x: Int32) -> Int32 {
    return x * 2
}

@_silgen_name("custom_c_name")
func swiftExportedWithCustomName(_ x: Int32) -> Int32 {
    return x + 1
}

@objc class ObjCCompatible: NSObject {
    @objc func objectiveCMethod() -> String {
        return "Hello from ObjC"
    }
}

@objc protocol ObjCProtocol {
    @objc func requiredMethod()
    @objc optional func optionalMethod()
}

func useCFunctions() {
    let ptr = malloc(100)
    free(ptr)
}
