def multilingual_greeting(name):
    """Say hello in multiple languages.

    この関数は多言語で挨拶します。
    Cette fonction salue en plusieurs langues.
    🎉 Celebrate with emoji! 🎉

    Args:
        name: 名前 / nom / name

    Returns:
        Dictionary of greetings
    """
    return {
        "en": f"Hello, {name}!",
        "ja": f"こんにちは、{name}さん！",
        "fr": f"Bonjour, {name}!",
    }
