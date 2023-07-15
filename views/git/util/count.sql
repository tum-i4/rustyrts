with counts as (SELECT commit.id, commit.repo_id,
       count(distinct suite.id) as suites,
       sum((SELECT count(distinct cas.id) FROM "TestCase" cas WHERE cas.suite_id = suite.id and cas.target = 'UNIT')) as unit,
      sum((SELECT count(distinct cas.id) FROM "TestCase" cas WHERE cas.suite_id = suite.id and cas.target = 'INTEGRATION')) as integration
FROM "Commit" commit,
     "TestReport" report,
     "TestSuite" suite
WHERE commit.id = report.commit_id
  AND report.has_errored = false
  AND report.name not like '%build%'
  and report.name not like '%parent%'
  and suite.report_id = report.id
GROUP BY commit.id,  commit.repo_id)

SELECT repo_id, avg(suites) as suites, avg(unit) as unit, avg(integration) as integration
FROM counts
GROUP BY repo_id;