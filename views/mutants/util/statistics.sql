-- this view calculates some statistics at the level of commits
create materialized view statistics_commit
AS
SELECT commit.id,
       commit.repo_id,
       commit.commit_str,
       commit.nr_lines                                as lines,
       commit.nr_files                                as files,
       count(distinct suite.id)                       as suites,
       sum((SELECT count(distinct cas.id)
            FROM "MutantsTestCase" cas
            WHERE cas.suite_id = suite.id
              and cas.status != 'IGNORED'))           as cases,
       sum((SELECT count(distinct cas.id)
            FROM "MutantsTestCase" cas
            WHERE cas.suite_id = suite.id
              and cas.target = 'UNIT'
              and cas.status != 'IGNORED'))           as unit,
       sum((SELECT count(distinct cas.id)
            FROM "MutantsTestCase" cas
            WHERE cas.suite_id = suite.id
              and cas.target = 'INTEGRATION'
              and cas.status != 'IGNORED'))           as integration,
       ROUND(CAST(sum(suite.duration) as numeric), 2) as duration
FROM "Commit" commit,
     "MutantsReport" report,
     "Mutant" mutant,
     "MutantsTestSuite" suite
WHERE commit.id = report.commit_id
  AND report.name = 'mutants'
  AND mutant.report_id = report.id
  AND mutant.descr = 'baseline'
  AND suite.mutant_id = mutant.id
GROUP BY commit.id, commit.repo_id;
