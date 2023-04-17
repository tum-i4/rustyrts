"""
Module containing base interfaces for SCM systems.
"""
import uuid
from abc import ABC, abstractmethod
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import List, Optional, Union, Dict

from ...util.logging.logger import get_logger

_LOGGER = get_logger(__name__)


class ChangelistItemAction(Enum):
    ADDED = "ADDED"
    MODIFIED = "MODIFIED"
    DELETED = "DELETED"
    NONE = "NONE"


class ChangelistItemKind(Enum):
    FILE = "FILE"
    DIR = "DIR"


class Repository(object):
    def __init__(self, path: str, repository_type: str) -> None:
        super().__init__()
        self.path = path
        self.repository_type = repository_type

    def __eq__(self, other: "Repository") -> bool:
        return self.path == other.path and self.repository_type == other.repository_type

    def __hash__(self) -> int:
        return hash("{}_{}".format(self.path, self.repository_type))


class ChangelistItem(object):
    def __init__(
            self,
            filepath: str,
            action: ChangelistItemAction,
            kind: ChangelistItemKind,
            content: Optional[str] = None
    ) -> None:
        """
        A constructor for a ChangelistItem.

        :param filepath:
        :param action:
        :param kind:
        :param content:
        """
        self.filepath = filepath
        self.action = action
        self.kind = kind
        self.content = content

    def __eq__(self, other: "ChangelistItem") -> bool:
        return self.filepath == other.filepath and self.action == other.action

    def __hash__(self) -> int:
        return hash("{}_{}".format(self.filepath, self.action))

    def __str__(self) -> str:
        return f"{self.action} {self.kind} {self.filepath}"

    def to_json(self) -> Dict:
        return {
            "filepath": self.filepath,
            "action": self.action.value,
            "kind": self.kind.value,
            "content": self.content
        }

    @classmethod
    def from_json(cls, json: Dict) -> "ChangelistItem":
        return cls(
            filepath=json["filepath"],
            action=ChangelistItemAction(json["action"]),
            kind=ChangelistItemKind(json["kind"]),
            content=json["content"]
        )

class Commit(object):
    def __init__(
        self,
        commit_str: str,
        author: str,
        message: str,
        timestamp: datetime,
        changelist: List[ChangelistItem],
        repo: Optional[Repository] = None,
        nr_lines = None,
        nr_files = None,
    ) -> None:
        self.author = author
        self.commit_str = commit_str
        self.message = message
        self.timestamp = timestamp
        self.changelist = changelist
        self.repo = repo
        self.nr_lines = nr_lines
        self.nr_files = nr_files

    @classmethod
    def create_virtual_commit(
        cls, changelist: List[ChangelistItem], repo: Optional[Repository] = None
    ) -> "Commit":
        now: datetime = datetime.now()
        return cls(
            commit_str="vc-{}".format(uuid.uuid4()),
            author="vc-author",
            message="vc-msg",
            timestamp=now,
            changelist=changelist,
            repo=repo,
        )

    def __eq__(self, other: "Commit") -> bool:
        return self.commit_str == other.commit_str

    def __hash__(self) -> int:
        return hash(self.commit_str)

    def __repr__(self) -> str:
        return self.commit_str

    def __lt__(self, other: "Commit") -> bool:
        return self.timestamp < other.timestamp

    def __str__(self) -> str:
        return self.commit_str

    def to_json(self) -> Dict:
        return {
            "author": self.author,
            "commit_str": self.commit_str,
            "message": self.message,
            "timestamp": self.timestamp.timestamp(),
            "changelist": [item.to_json() for item in self.changelist],
            "nr_lines": self.nr_lines,
            "nr_files": self.nr_files,
        }

    @classmethod
    def from_json(cls, json: Dict) -> "Commit":
        return cls(
            author=json["author"],
            commit_str=json["commit_str"],
            message=json["message"],
            timestamp=datetime.fromtimestamp(int(json["timestamp"])),
            changelist= [ChangelistItem.from_json(item) for item in json["items"]],
            nr_lines=json["nr_lines"],
            nr_files=json["nr_files"],
        )


class SCMClient(ABC):
    @staticmethod
    def get_client_for_path(path: str) -> Optional["SCMClient"]:
        # add imports here to prevent cyclic deps
        from ...util.scm.git import is_git_repo
        from .git import GitClient

        scm_client: Optional["SCMClient"] = None
        if is_git_repo(path):
            _LOGGER.debug("Found git repository.")
            scm_client = GitClient.create_client(path=path)
        return scm_client

    @abstractmethod
    def get_repository(self) -> Repository:
        pass

    @abstractmethod
    def get_diff(
        self,
        from_revision: Union[int, str],
        to_revision: Optional[Union[int, str]] = None,
    ) -> List[ChangelistItem]:
        """
        Get a combined changelist depicting the diff between two revisions.
        :param from_revision:
        :param to_revision:
        :return:
        """
        pass

    @abstractmethod
    def get_file_content_at_commit(
        self, revision: Union[int, str], file_path: Path
    ) -> str:
        """
        Get the content of a file at a certain revision.

        :param revision: Revision or branch name at which to get the content of the file.
        :param file_path: File path relative to repository root.
        :return:
        """
        pass

    @abstractmethod
    def get_status(
        self,
    ) -> List[ChangelistItem]:
        """
        Get a changelist that contains all currently changed/added/deleted files.
        :return:
        """
        pass