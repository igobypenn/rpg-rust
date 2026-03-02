package com.example.app

import scala.collection.mutable

case class Config(
  name: String,
  version: String = "1.0.0",
  settings: mutable.Map[String, String] = mutable.Map.empty
) {
  def set(key: String, value: String): Config = {
    settings(key) = value
    this
  }

  def get(key: String): Option[String] = settings.get(key)

  def process(data: List[String]): String = data.mkString
}

class DataProcessor(private val config: Config) {
  def run(input: String): String = {
    val parts = input.map(_.toString).toList
    config.process(parts)
  }
}

trait Repository[T] {
  def find(id: Long): Option[T]
  def save(entity: T): Boolean
  def delete(id: Long): Boolean
}

case class User(id: Long, name: String, email: String)

object UserRepository extends Repository[User] {
  private val users = mutable.Map.empty[Long, User]

  override def find(id: Long): Option[User] = users.get(id)
  override def save(entity: User): Boolean = { users(entity.id) = entity; true }
  override def delete(id: Long): Boolean = users.remove(id).isDefined
}

object Factory {
  def createConfig(name: String): Config = Config(name)
}

object Main extends App {
  val config = Factory.createConfig("test")
  println(config.name)
}
