from abc import ABC, abstractmethod
from typing import List, Optional

from rustyrts_eval.models.scm.git import GitClient

from ...models.scm.base import Repository, Commit


class Hook(ABC):
    """
    Hook interface for execution inside walkers.
    """

    def __init__(
        self,
        repository: Repository,
        output_path: Optional[str],
        git_client: GitClient,
    ):
        super().__init__()
        self.repository = repository
        self.output_path = output_path
        self.git_client = git_client

    @abstractmethod
    def run(
        self,
        commit: Commit,
        individual_options_parent: tuple[Optional[str], Optional[str]],
        individual_options: tuple[Optional[str], Optional[str]],
    ) -> int:
        pass


class Walker(ABC):
    """
    Walker base class to replay repository history.
    """

    def __init__(
        self,
        repository: Repository,
        strategy: "WalkerStrategy",
        num_commits: Optional[int] = 10,
        hooks: Optional[List[Hook]] = None,
    ):
        """
        Constructor for walkers.

        :param repository: A **local** git repository.
        :param strategy: The strategy used in selecting commits
        :param num_commits:
        :param hooks:
        """
        super().__init__()
        self.repository = repository
        self.strategy = strategy
        self.num_commits = num_commits
        self.hooks: List[Hook] = hooks if hooks else []

    @abstractmethod
    def walk(self) -> None:
        """
        Step through the repository history and execute hooks before, while and after stepping.
        """
        pass


class WalkerStrategy(ABC):
    def __int__(self):
        pass

    @abstractmethod
    def __iter__(self):
        pass
