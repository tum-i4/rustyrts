import pandas as pd
import seaborn as sns
import matplotlib as mpl
import matplotlib.pyplot as plt


def boxplot(df, labels, y_label, file, palette=None, hue='algorithm', figsize=(20, 15), single_threaded=False):
    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.boxplot(data=df,
                     x='repository',
                     y='y',
                     hue=hue,
                     showmeans=True,
                     width=0.75,
                     meanprops={
                         "marker": "v",
                         "markerfacecolor": "white",
                         "markeredgecolor": "black",
                         "markersize": "16"
                     },
                     fliersize=14,
                     palette=palette)
    ax.set_xticklabels(labels=labels, rotation='vertical')
    ax.set_xlabel("")
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which='major', linewidth=1.0)
    ax.grid(which='minor', linewidth=0.5)
    if single_threaded:
        plt.figtext(0.01,0.02, 'single-threaded', color='grey', rotation="vertical")
    plt.legend(loc='best')
    plt.tight_layout(pad=0.2)
    plt.savefig(file)


def boxplot_with_observations(df, labels, y_label, file, palette=None, hue='algorithm', figsize=(20, 15),
                              single_threaded=False):
    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.boxplot(data=df,
                     x='repository',
                     y='y',
                     hue=hue,
                     showmeans=True,
                     width=0.75,
                     meanprops={
                         "marker": "v",
                         "markerfacecolor": "white",
                         "markeredgecolor": "black",
                         "markersize": "16"
                     },
                     fliersize=14,
                     palette=palette)

    sns.stripplot(ax=ax,
                  data=df,
                  x='repository',
                  y='y',
                  hue=hue,
                  dodge=True,
                  jitter=.3,
                  size=8,
                  linewidth=1,
                  palette=palette,
                  legend=False)

    ax.set_xticklabels(labels=labels, rotation='vertical')
    ax.set_xlabel("")
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which='major', linewidth=1.0)
    ax.grid(which='minor', linewidth=0.5)
    if single_threaded:
        plt.figtext(0.01,0.02, 'single-threaded', color='grey', rotation="vertical")
    plt.legend(loc='best')
    plt.tight_layout(pad=0.2)
    plt.savefig(file)


def barplot(df, labels, y_label, file, palette, hue='algorithm', figsize=(20, 15), single_threaded=False):
    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.barplot(data=df,
                     x='repository',
                     y='y',
                     hue=hue,
                     # showmeans=True,
                     # width=0.75,
                     # meanprops={
                     #    "marker": "v",
                     #    "markerfacecolor": "white",
                     #    "markeredgecolor": "black",
                     #    "markersize": "8"
                     # },
                     palette=palette)
    ax.set_xticklabels(labels=labels)
    ax.set_xlabel("")
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which='major', linewidth=1.0)
    ax.grid(which='minor', linewidth=0.5)
    if single_threaded:
        plt.figtext(0.01,0.02, 'single-threaded', color='grey', rotation="vertical")
    plt.legend(loc='best')
    plt.tight_layout(pad=0.2)
    plt.savefig(file)


def stripplot(df, labels, y_label, file, palette=None, hue='algorithm', figsize=(20, 15), legend=True,
              legend_loc='best', legend_anchor=None, single_threaded=False):
    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.stripplot(data=df,
                       x='repository',
                       y='y',
                       hue=hue,
                       dodge=True,
                       jitter=.3,
                       size=8,
                       linewidth=1,
                       palette=palette)
    ax.set_xticklabels(labels=labels, rotation='vertical')
    ax.set_xlabel("")
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which='major', linewidth=1.0)
    ax.grid(which='minor', linewidth=0.5)
    if legend:
        if legend_anchor:
            plt.legend(loc=legend_loc, bbox_to_anchor=legend_anchor)
        else:
            plt.legend(loc=legend_loc, bbox_to_anchor=legend_anchor)
    else:
        plt.legend([], [], frameon=False)
    if single_threaded:
        plt.figtext(0.01,0.02, 'single-threaded', color='grey', rotation="vertical")
    plt.tight_layout(pad=0.2)
    plt.savefig(file)


def scatterplot(df_raw, labels, x_label, y_label, file, palette=None, hue='algorithm', figsize=(20, 15),
                x_scale='linear',
                y_scale='linear', legend=True,
                legend_loc='best', legend_anchor=None, single_threaded=False):
    df = pd.concat(df_raw)

    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.scatterplot(data=df,
                         x='x',
                         y='y',
                         hue=hue,
                         palette=palette)

    for i in range(len(df_raw)):
        ax = sns.regplot(
            data=df_raw[i], x="x", y="y", logx=True, label=labels[i],
            scatter=False, truncate=False, order=1, color=palette[i],
        )

    ax.set_xscale(x_scale)
    ax.set_yscale(y_scale)

    ax.set_xlabel(x_label)
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.get_xaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which='major', linewidth=1.0)
    ax.grid(which='minor', linewidth=0.5)
    if legend:
        if legend_anchor:
            plt.legend(loc=legend_loc, bbox_to_anchor=legend_anchor)
        else:
            plt.legend(loc=legend_loc, bbox_to_anchor=legend_anchor)
    else:
        plt.legend([], [], frameon=False)
    if single_threaded:
        plt.figtext(0.01,0.02, 'single-threaded', color='grey')
    plt.tight_layout(pad=0.2)
    plt.savefig(file)
