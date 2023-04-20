from typing import List, Optional

from sqlalchemy import Column, String, Integer, ForeignKey, DateTime, Enum
from sqlalchemy.orm import relationship, Session

from .base import Base
from ..models.scm.base import Commit, ChangelistItemKind, ChangelistItemAction, Repository, ChangelistItem


########################################################################################################################
# Meta classes
#

class DBRepositoryMeta(Base.__class__, Repository.__class__):
    ...


class DBCommitMeta(Base.__class__, Commit.__class__):
    ...


class DBChangelistItemMeta(Base.__class__, ChangelistItem.__class__):
    ...


########################################################################################################################
# Actual db classes
#

class DBRepository(Base, Repository, metaclass=DBRepositoryMeta):
    path = Column(String, unique=True, index=True, nullable=False)
    repository_type = Column(String)
    commits: List[Commit] = relationship("DBCommit", back_populates="repo")

    @classmethod
    def create(cls, repo: Repository, session: Session) -> "DBRepository":
        # create db object
        db_repo = DBRepository.from_domain(repo)

        # create in session
        session.add(db_repo)

        return db_repo

    @classmethod
    def create_or_get(cls, repo: Repository, session: Session) -> "DBRepository":
        # get repo from DB
        db_repo: DBRepository | None = session.query(DBRepository).filter_by(path=repo.path).first()
        if not db_repo:
            db_repo = cls.create(repo=repo, session=session)

        return db_repo

    @classmethod
    def create_or_update(cls, repo: Repository, session: Session) -> "DBRepository":
        # get repo from DB
        db_repo: DBRepository | None = session.query(DBRepository).filter_by(path=repo.path).first()
        if not db_repo:
            return cls.create(repo=repo, session=session)

        # update properties
        db_repo.repository_type = repo.repository_type
        session.commit()
        return db_repo

    @classmethod
    def from_domain(cls, repository: Repository) -> "DBRepository":
        if isinstance(repository, cls) or not repository:
            return repository
        return cls(path=repository.path, repository_type=repository.repository_type)

    def to_domain(self) -> Repository:
        return Repository(path=self.path, repository_type=self.repository_type)


class DBCommit(Base, Commit, metaclass=DBCommitMeta):
    commit_str = Column(String, nullable=False)
    author = Column(String)
    message = Column(String)
    timestamp = Column(DateTime)
    changelist: List["DBChangelistItem"] = relationship(
        "DBChangelistItem", back_populates="commit"
    )
    repo_id = Column(Integer, ForeignKey("{}.id".format(DBRepository.__tablename__), ondelete="CASCADE"))
    repo: Optional[DBRepository] = relationship(
        "DBRepository", back_populates="commits"
    )
    reports = relationship("DBTestReport", back_populates="commit")
    mutants_reports = relationship("DBMutantsReport", back_populates="commit")
    nr_lines = Column(Integer, nullable=True)
    nr_files = Column(Integer, nullable=True)

    @classmethod
    def create(cls, commit: Commit, session: Session) -> "DBCommit":
        if commit.repo:
            # get repo if exists, otherwise create
            commit.repo = DBRepository.create_or_get(commit.repo, session)

        # create db object
        db_commit = DBCommit.from_domain(commit)

        # create in session
        session.add(db_commit)

        return db_commit

    @classmethod
    def create_or_get(cls, commit: Commit, session: Session) -> "DBCommit":
        # get commit from DB
        db_commit: DBCommit | None = (
            session.query(DBCommit).filter_by(commit_str=commit.commit_str).first()
        )
        # create DB commit object if not in DB yet
        if not db_commit:
            db_commit = cls.create(commit=commit, session=session)
        return db_commit

    @classmethod
    def create_or_update(cls, commit: Commit, session: Session) -> "DBCommit":
        # get commit from DB
        db_commit: DBCommit | None = (
            session.query(DBCommit).filter_by(commit_str=commit.commit_str).first()
        )
        # create DB commit object if not in DB yet
        if not db_commit:
            return cls.create(commit=commit, session=session)

        if db_commit.repo != commit.repo:
            db_commit.repo = DBRepository.create_or_update(
                repo=commit.repo, session=session
            )

        db_commit.author = commit.author
        db_commit.message = commit.message
        db_commit.timestamp = commit.timestamp
        if db_commit.changelist != commit.changelist:
            [DBChangelistItem.from_domain(item) for item in commit.changelist]
        db_commit.nr_lines = commit.nr_lines
        db_commit.nr_files = commit.nr_files

        session.commit()
        return db_commit

    @classmethod
    def from_domain(cls, commit: Commit) -> "DBCommit":
        if isinstance(commit, cls) or not commit:
            return commit
        return cls(
            commit_str=commit.commit_str,
            author=commit.author,
            message=commit.message,
            timestamp=commit.timestamp,
            changelist=[DBChangelistItem.from_domain(item) for item in commit.changelist],
            repo=DBRepository.from_domain(commit.repo),
            nr_lines=commit.nr_lines,
            nr_files=commit.nr_files
        )

    def to_domain(self) -> Commit:
        return Commit(
            commit_str=self.commit_str,
            author=self.author,
            message=self.message,
            timestamp=self.timestamp,
            changelist=[item.to_domain() for item in self.changelist],
            repo=self.repo.to_domain() if self.repo else None,
            nr_lines=self.nr_lines,
            nr_files=self.nr_files
        )


class DBChangelistItem(Base, ChangelistItem, metaclass=DBChangelistItemMeta):
    filepath = Column(String, nullable=False)
    action = Column(Enum(ChangelistItemAction))
    kind = Column(Enum(ChangelistItemKind))
    content = Column(String)
    commit_id = Column(
        Integer, ForeignKey("{}.id".format(DBCommit.__tablename__), ondelete="CASCADE")
    )
    commit = relationship("DBCommit", back_populates="changelist")

    @classmethod
    def from_domain(cls, changelist_item: ChangelistItem) -> "DBChangelistItem":
        if isinstance(changelist_item, cls) or not changelist_item:
            return changelist_item
        return cls(
            filepath=changelist_item.filepath,
            action=changelist_item.action,
            kind=changelist_item.kind,
            content=changelist_item.content
        )

    def to_domain(self) -> ChangelistItem:
        return ChangelistItem(
            filepath=self.filepath,
            action=self.action,
            kind=self.kind,
            content=self.content
        )
