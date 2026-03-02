package com.example.ffi

import com.sun.jna.{Native, Library, Pointer}
import scala.scalanative.unsafe._
import scala.scalanative.unsigned._

trait MyLib extends Library {
  def add_numbers(a: Int, b: Int): Int
  def process_data(data: Pointer, len: Long): Int
}

object JnaExample {
  val lib = Native.load("mylib", classOf[MyLib])

  def addViaJna(a: Int, b: Int): Int = lib.add_numbers(a, b)
}

object JniExample {
  @native def nativeAdd(a: Int, b: Int): Int
  @native def nativeProcess(data: Array[Byte]): Int

  System.loadLibrary("mylib")
}

@extern object CStdLib {
  def malloc(size: CSize): Ptr[Byte] = extern
  def free(ptr: Ptr[Byte]): Unit = extern
  def strlen(s: CString): CSize = extern
}

object ScalaNativeExample {
  def useNative(): CSize = {
    val str = c"Hello"
    CStdLib.strlen(str)
  }
}

@exported def exportedFunction(x: Int): Int = x * 2
