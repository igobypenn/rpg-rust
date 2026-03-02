import os
import sys
from typing import List, Dict, Optional


class DataProcessor:
    """A class for processing data."""

    def __init__(self, name: str):
        self.name = name
        self.config: Dict[str, str] = {}

    def process(self, data: List[str]) -> Optional[str]:
        if not data:
            return None
        return "".join(data)

    def configure(self, key: str, value: str) -> None:
        self.config[key] = value


def create_processor(name: str) -> DataProcessor:
    return DataProcessor(name)


def merge_dicts(base: Dict[str, str], override: Dict[str, str]) -> Dict[str, str]:
    result = base.copy()
    result.update(override)
    return result
