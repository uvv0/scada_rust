#ifndef EX_H
#define EX_H

#include <QMainWindow>
#include "ser.h"
#include "ob.h"
#include "ud.h"
#include "object_programming.h"
#include "tag_config_protocol.h"
#include <QVector>

class QWidget;
class QTableWidget;
class QLabel;
class QPushButton;
class QCheckBox;
class QComboBox;
class QLineEdit;
class QSpinBox;
class QDoubleSpinBox;
class QProgressBar;
class QTextEdit;
class QNetworkAccessManager;
class QNetworkReply;
//#include "di1.h"
namespace Ui {
class ex;
}

class ex : public QMainWindow
{
    Q_OBJECT

public:
    explicit ex(QWidget *parent = 0);
    ~ex();
    //Di1 dd;
    ser w;
    ob w2;
    QTimer *dds2;
     QStringList ss;
     QStringList ss1;
     u16 ind_st_zz,ind_st_zz1,ind_t1;
     void sh(bool par);
     void sht(int k);
      private slots:
    void on_comboBox_6_currentIndexChanged(int index);

    void on_pushButton_166_clicked();

    void on_pushButton_167_clicked();

    void on_pushButton_171_clicked();

    void on_pushButton_174_clicked();

    void on_pushButton_172_clicked();

    void on_comboBox_currentIndexChanged(int index);

    void on_comboBox_3_currentIndexChanged(const QString &arg1);

    void on_pushButton_clicked();

    void on_pushButton_27_clicked();

    void on_pushButton_41_clicked();

    void on_checkBox_28_clicked();
    void sh_ts();
    void sh_csq();
    void sh_s_sim();
    void sh_ip();
    void sh_tii3();
    void sh_tit12();

    void on_pushButton_40_clicked();

    void on_pushButton_43_clicked();
    void sh_tit_float();

    void on_pushButton_2_clicked();

    void on_lineEdit_118_textChanged(const QString &arg1);

    void on_lineEdit_118_editingFinished();

    //void on_pushButton_4_clicked();

    //void on_lineEdit_14_cursorPositionChanged(int arg1, int arg2);

    //void on_pushButton_13_clicked();

    void on_get_ver_rd_clicked();
    //void set_func(void (*f)(void));
    void sh_ver();

    void on_ini_par_ty_clicked();

    void on_time_rd_clicked();
    void sh_d();

    void on_time_wr_clicked();

    void on_time_set_2_clicked();

    void on_set_t1_rd_clicked();
    void sh_tii_time();

    void on_set_t1_wr_clicked();

    void on_pushButton_51_clicked();

    void on_tit_rd_clicked();
    void sh_n_f();

    void on_tit_rd_2_clicked();
    void sh_tii_time2();

    void on_tit_setd_clicked();

    void on_pushButton_278_clicked();
    void sh_mbF();

    void on_pushButton_279_clicked();

    void on_B_sos_clicked();
    void sh_sos2();

    void on_B_zam_clicked();
    void sh_zamb();

    void on_pushButton_283_clicked();
    void sh_er2();

    void on_pushButton_139_clicked();
    void sh_indzz(void);


    void on_pushButton_140_clicked();
    void sh_arx_zz(void);

    void on_pushButton_145_clicked();
    void sh_indzz1(void);

    void on_pushButton_146_clicked();
    void sh_arx_zz1(void);

    void on_pushButton_144_clicked();

    void on_pushButton_280_clicked();

    void on_pushButton_141_clicked();

    void on_pushButton_142_clicked();

    void on_pushButton_149_clicked();

    void on_pushButton_147_clicked();

    void on_pushButton_148_clicked();

    void on_p_ind1_clicked();
    void sh_ind_t1(void); //

    void on_pushButton_153_clicked();
    void sh_arx_t1(void);

    void on_l_t1_textChanged(const QString &arg1);
    void sh_ty();

    void on_ini_par_ty_2_clicked();

    void on_pushButton_52_clicked();

    void on_p_bl_on_clicked();

    void on_p_bl_onn_clicked();

    void on_p_bl_off_clicked();

    void on_p_poll_on_clicked();

    void on_p_poll_off_clicked();

    void on_p_sos_slot_clicked();
    void sh_ssos(void);
    void on_p_sos_slot_2_clicked();
    void sh_s2500(void);
    void objectProgramOpen();
    void objectProgramWrite();
    void objectProgramStart();
    void objectProgramStop();
    void objectProgramStatus();
    void objectProgramCatalog();
    void objectCatalogSelected(int index);
    void objectProgramCancel();
    void objectProgramResponse();
    void objectProgramPoll();
    void objectProgramTimeout();
    void objectProgramTypeChanged(int index);
    void profilerPoll();
    void profilerResponse();
    void profilerTimeout();
    void profilerRefresh();
    void profilerEnable();
    void profilerEnableResponse();
    void profilerAutoChanged(bool enabled);
    void profilerTabChanged(int index);
    void slot1Poll();
    void slot1Response();
    void slot1Timeout();
    void slot1Refresh();
    void slot1AutoChanged(bool enabled);
    void slot1TabChanged(int index);
    void floatConfigRead();
    void floatConfigWrite();
    void floatConfigResponse();
    void floatConfigTimeout();
    void tagConfigRead();
    void tagConfigSave();
    void tagConfigActivate();
    void tagConfigResponse();
    void tagConfigTimeout();
    void tagConfigPoll();
    void tagConfigSelectionChanged();
    void luaRead();
    void luaWrite();
    void luaComWrite();
    void luaComRead();
    void luaSlotChanged(int slot);
    void luaRun();
    void luaStop();
    void luaWriteAndRun();
    void luaStatus();
    void luaSlotsStatus();
    void luaSlotsStatusResponse();
    void luaSlotsStatusTimeout();
    void luaRefreshTags();
    void luaTagsValuesResponse();
    void luaTagsValuesTimeout();
    void luaTagsAutoRefresh();
    void luaTagsContinueRead();
    void luaReplyFinished(QNetworkReply *reply);
    void luaInsertTag(int row, int column);
    void on_slot1RefreshButton_clicked();

    void on_floatConfigReadButton_clicked();

private:
    enum ObjectProgramPhase
    {
        ObjectProgramIdle,
        ObjectProgramBeginConfig,
        ObjectProgramBeginCommand,
        ObjectProgramBeginPoll,
        ObjectProgramChunkData,
        ObjectProgramChunkMeta,
        ObjectProgramChunkCommand,
        ObjectProgramChunkPoll,
        ObjectProgramCommitCommand,
        ObjectProgramCommitPoll,
        ObjectProgramAbortCommand,
        ObjectProgramReplaceAbortCommand,
        ObjectProgramReplaceAbortPoll,
        ObjectProgramReplaceSelect,
        ObjectProgramReplaceCommand,
        ObjectProgramReplacePoll,
        ObjectProgramStartSelect,
        ObjectProgramStartCommand,
        ObjectProgramStartPoll,
        ObjectProgramStopSelect,
        ObjectProgramStopCommand,
        ObjectProgramStopPoll,
        ObjectProgramStatusSelect,
        ObjectProgramStatusCommand,
        ObjectProgramStatusPoll,
        ObjectProgramCatalogSelect,
        ObjectProgramCatalogCommand,
        ObjectProgramCatalogPoll,
        ObjectProgramCatalogHeader,
        ObjectProgramLuaReadSelect,
        ObjectProgramLuaReadCommand,
        ObjectProgramLuaReadPoll,
        ObjectProgramLuaReadData
    };

    void setupObjectProgrammingPage(bool portAvailable);
    void objectProgramSend(const QByteArray &frame);
    void objectProgramSchedulePoll();
    void objectProgramStartChunk();
    void objectProgramFinish(const QString &message, bool success);
    void objectProgramFail(const QString &message);
    void objectProgramLog(const QString &message);
    void objectProgramRuntime(ObjectProgramPhase phase,
                              const QString &message);
    void setupProfilerPage();
    void profilerStartCycle();
    void profilerRead(unsigned int address, unsigned int count);
    void profilerFinish(bool success, const QString &message);
    void profilerRender();
    static quint32 profilerU32(const QVector<quint16> &data, int offset);
    static quint64 profilerU64(const QVector<quint16> &data, int offset);
    void setupSlot1Page();
    void slot1StartCycle();
    void slot1Finish(bool success, const QString &message);
    void setupFloatConfigPage();
    void floatConfigFinish(bool success, const QString &message);
    void setupTagConfigPage();
    void tagConfigStart(quint16 command);
    void tagConfigSend(const QByteArray &frame);
    void tagConfigFinish(bool success, const QString &message);
    QVector<TagConfigProtocol::Row> tagConfigRows() const;
    void tagConfigRender(const QVector<TagConfigProtocol::Row> &rows);
    void setupLuaPage();
    void luaRequest(const QString &operation);
    void luaSetBusy(bool busy);
    void luaShowStatus(const QString &message, bool success);
    void luaStartNextTagValueRead();
    void luaStopTagRefresh();

    enum TagConfigPhase
    {
        TagConfigIdle,
        TagConfigUpload,
        TagConfigControl,
        TagConfigStatus,
        TagConfigDownload
    };

    QWidget *objectProgramTab;
    QLineEdit *objectFileEdit;
    QLineEdit *objectNameEdit;
    QLineEdit *objectLinkAddressEdit;
    QComboBox *objectTypeCombo;
    QComboBox *objectContentTypeCombo;
    QSpinBox *objectIdSpin;
    QSpinBox *objectApiVersionSpin;
    QSpinBox *objectEntryOffsetSpin;
    QCheckBox *objectAutostartCheck;
    QCheckBox *objectReadonlyCheck;
    QCheckBox *objectSystemCheck;
    QCheckBox *objectCompressedCheck;
    QLabel *objectContentTypeLabel;
    QLabel *objectLinkAddressLabel;
    QLabel *objectEntryOffsetLabel;
    QLabel *objectApiVersionLabel;
    QPushButton *objectOpenButton;
    QPushButton *objectWriteButton;
    QPushButton *objectCancelButton;
    QPushButton *objectStartButton;
    QPushButton *objectStopButton;
    QPushButton *objectStatusButton;
    QPushButton *objectCatalogButton;
    QComboBox *objectCatalogCombo;
    QProgressBar *objectProgress;
    QTextEdit *objectLog;
    QByteArray objectProgramPayload;
    QByteArray objectProgramImage;
    QByteArray objectProgramChunk;
    QList<QByteArray> objectProgramFrames;
    QByteArray objectProgramLastRequest;
    ObjectProgramming::ImageInfo objectProgramImageInfo;
    ObjectProgramPhase objectProgramPhase;
    int objectProgramFrameIndex;
    int objectProgramPollCount;
    int objectProgramOffset;
    int objectCatalogIndex;
    ObjectProgramming::Status objectCatalogPendingStatus;
    QList<ObjectProgramming::ObjectInfo> objectCatalogObjects;
    bool objectProgramCancelled;
    QTimer *objectProgramTimeoutTimer;
    QWidget *profilerTab;
    QTableWidget *profilerTable;
    QLabel *profilerStatus;
    QLabel *profilerLoad;
    QLabel *profilerWindow;
    QPushButton *profilerRefreshButton;
    QPushButton *profilerEnableButton;
    QCheckBox *profilerAuto;
    QTimer *profilerPollTimer;
    QTimer *profilerTimeoutTimer;
    QVector<quint16> profilerHeader;
    QVector<quint16> profilerThreadData;
    unsigned int profilerRequestAddress;
    unsigned int profilerRequestCount;
    int profilerThreadCount;
    int profilerNextRegister;
    bool profilerBusy;
    bool profilerEnableActive;
    bool profilerReadingHeader;
    QWidget *slot1Tab;
    QLabel *slot1Status;
    QLabel *slot1Value;
    QLabel *slot1Raw;
    QLabel *slot1Comm;
    QLabel *slot1Success;
    QPushButton *slot1RefreshButton;
    QCheckBox *slot1Auto;
    QTimer *slot1PollTimer;
    QTimer *slot1TimeoutTimer;
    bool slot1Busy;
    QWidget *floatConfigTab;
    QTableWidget *floatConfigTable;
    QLabel *floatConfigStatus;
    QPushButton *floatConfigReadButton;
    QPushButton *floatConfigWriteButton;
    QTimer *floatConfigTimeoutTimer;
    bool floatConfigBusy;
    bool floatConfigWriting;
    QWidget *tagConfigTab;
    QTableWidget *tagConfigTable;
    QLabel *tagConfigStatus;
    QLabel *tagConfigGenerationLabel;
    QSpinBox *tagConfigPortSpin;
    QSpinBox *tagConfigDeviceSpin;
    QPushButton *tagConfigReadButton;
    QPushButton *tagConfigSaveButton;
    QPushButton *tagConfigActivateButton;
    QTimer *tagConfigTimeoutTimer;
    TagConfigPhase tagConfigPhase;
    quint16 tagConfigCommand;
    quint32 tagConfigGeneration;
    quint8 tagConfigActivePort;
    quint8 tagConfigActiveDevice;
    int tagConfigFrameIndex;
    int tagConfigPollCount;
    int tagConfigExpectedLength;
    QList<QByteArray> tagConfigFrames;
    QByteArray tagConfigBlob;
    QByteArray tagConfigLastRequest;
    QWidget *luaTab;
    QLineEdit *luaHostEdit;
    QSpinBox *luaObjectIdSpin;
    QSpinBox *luaComSlotSpin;
    QSpinBox *luaVmIdSpin;
    QLineEdit *luaObjectNameEdit;
    QTextEdit *luaSourceEdit;
    QLabel *luaStatusLabel;
    QTableWidget *luaSlotsTable;
    QTableWidget *luaTagsTable;
    QPushButton *luaReadButton;
    QPushButton *luaWriteButton;
    QPushButton *luaComWriteButton;
    QPushButton *luaComReadButton;
    QPushButton *luaRunButton;
    QPushButton *luaStopButton;
    QPushButton *luaWriteRunButton;
    QPushButton *luaStatusButton;
    QPushButton *luaSlotsStatusButton;
    QPushButton *luaTagsButton;
    QNetworkAccessManager *luaNetwork;
    QString luaPendingOperation;
    bool luaRunAfterWrite;
    bool luaComWriteActive;
    bool luaComReadActive;
    bool luaRuntimeActive;
    bool luaTagsRefreshActive;
    bool luaRefreshTagsAfterRead;
    QTimer *luaTagsValuesTimer;
    QTimer *luaTagsAutoTimer;
    QTimer *luaSlotsStatusTimer;
    QByteArray luaComReadPayload;
    QList<quint32> luaTagKeys;
    int luaTagKeyIndex;
    int luaComReadExpected;
    int luaComReadOffset;
    Ui::ex *ui;
};

#endif // EX_H
