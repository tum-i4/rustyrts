from abc import ABC, abstractmethod


class TestReportLoader(ABC):
    """
    A generic loader interface to load test reports from different data sources.
    An example is the JenkinsTestReportLoader in `models.testing.impl.jenkins`.
    """

    @abstractmethod
    def load(self):
        pass
