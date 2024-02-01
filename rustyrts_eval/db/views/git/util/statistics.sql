-- this view calculates some statistics at the level of commits
--create materialized view statistics_commit AS
SELECT commit.id,
       commit.repo_id,
       commit.nr_lines                                as lines,
       commit.nr_files                                as files,
       count(distinct suite.id)                       as suites,
       sum((SELECT count(distinct cas.id)
            FROM "TestCase" cas
            WHERE cas.suite_id = suite.id
              and cas.status != 'IGNORED'))           as cases,
       sum((SELECT count(distinct cas.id)
            FROM "TestCase" cas
            WHERE cas.suite_id = suite.id
              and cas.target = 'UNIT'
              and cas.status != 'IGNORED'))           as unit,
       sum((SELECT count(distinct cas.id)
            FROM "TestCase" cas
            WHERE cas.suite_id = suite.id
              and cas.target = 'INTEGRATION'
              and cas.status != 'IGNORED'))           as integration,
       ROUND(CAST(sum(suite.duration) as numeric), 2) as duration
FROM "Commit" commit,
     "TestReport" report,
     "TestSuite" suite
WHERE commit.id = report.commit_id
  AND report.has_errored = false
  AND report.name = 'cargo test'
  and suite.report_id = report.id
GROUP BY commit.id, commit.repo_id;


-- this view calculates some statistics at the level of repositories
create materialized view statistics_repository
AS
SELECT repo_id,
       ROUND(CAST(avg(lines) as numeric), 2)       as avg_lines,
       ROUND(CAST(avg(files) as numeric), 2)       as avg_files,
       ROUND(CAST(avg(suites) as numeric), 2)      as avg_suites,
       ROUND(CAST(avg(cases) as numeric), 2)       as avg_cases,
       ROUND(CAST(avg(unit) as numeric), 2)        as avg_unit,
       ROUND(CAST(avg(integration) as numeric), 2) as avg_integration,
       ROUND(CAST(avg(duration) as numeric), 2)    as avg_duration
FROM statistics_commit
GROUP BY repo_id
--;