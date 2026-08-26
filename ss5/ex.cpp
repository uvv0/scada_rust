#include "ex.h"
#include "ui_ex.h"
#include <QFile>
#include <QFileInfo>
#include <QFileDialog>
#include "qextserialenumerator.h"
#include <QCheckBox>
#include <QComboBox>
#include <QDateTime>
#include <QDoubleSpinBox>
#include <QHeaderView>
#include <QHBoxLayout>
#include <QGridLayout>
#include <QGroupBox>
#include <QLabel>
#include <QLineEdit>
#include <QProgressBar>
#include <QPushButton>
#include <QSpinBox>
#include <QTableWidget>
#include <QTextEdit>
#include <QVBoxLayout>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QUrl>
#include <QRegExp>
#include <QDir>
#include <QCoreApplication>

ex::ex(QWidget *parent) :
    QMainWindow(parent),
    ui(new Ui::ex)
{
    ui->setupUi(this);
    ui->comboBox->blockSignals(true);
    ui->comboBox->addItems(QStringList() << "4800" << "9600" <<"14400" << "19200" << "38400" << "57600"<<"115200");
    ui->comboBox->setCurrentIndex(3);
    ui->comboBox->blockSignals(false);
    ui->spinBox_8->setValue(w.mb_rtu);

    QTextCodec *codec = QTextCodec::codecForName("UTF-8");
    QTextCodec::setCodecForTr(codec);
    QTextCodec::setCodecForCStrings(codec);
    w.Baud1=BAUD19200;
    w.mb_rtu =301;
    ui->comboBox_3->blockSignals(true);
    ui->comboBox_3->clear();
    const QList<QextPortInfo> ports = QextSerialEnumerator::getPorts();
    int preferredPort = -1;
    for (int portIndex = 0; portIndex < ports.size(); ++portIndex)
    {
        const QString displayName = ports.at(portIndex).portName;
        QString deviceName = displayName;
        if (displayName.startsWith("COM", Qt::CaseInsensitive) &&
            displayName.mid(3).toInt() > 9)
            deviceName = "\\\\.\\" + displayName;
        ui->comboBox_3->addItem(displayName, deviceName);
        if (displayName.compare("COM6", Qt::CaseInsensitive) == 0)
            preferredPort = portIndex;
    }
    if (ports.isEmpty())
    {
        ui->comboBox_3->addItem(tr("Нет COM-портов"));
        ui->comboBox_3->setEnabled(false);
        statusBar()->showMessage(tr("В системе не найдено COM-портов"));
    }
    else
    {
        if (preferredPort < 0)
            preferredPort = 0;
        ui->comboBox_3->setCurrentIndex(preferredPort);
        w.port_name = ui->comboBox_3->itemData(preferredPort).toString();
    }
    ui->comboBox_3->blockSignals(false);
    if (!ports.isEmpty())
        w.ini_c();
    ui->spinBox_8->setValue(w.mb_rtu);
    w.enn = 0;
    QStringList  gg;
    gg << "нет арх"<< "1 мин"<< "2 мин"<< "5 мин" << "10 мин" << "20 мин" <<
         "30 мин" << "1 час"<<"2 часа"<<"4 часа"<<" 6 часов"<<"8 часов"<<"12 часов"<<"24 часа"
          <<"10 секунд"<<"20 секунд"<<"30 секунд";
    ui->set_t1_box->addItems(gg);
    ui->set_t2_box->addItems(gg);
    //dds2 = new QTimer(this);
    //connect(dds2, SIGNAL(timeout()),this, SLOT(rec2()));
    //dds2->start(700);
    for(int k=0;k<12;k++){
     ui->table_kp->setItem(k,0,new QTableWidgetItem(QString::number(k)));
     ui->table_kp->setItem(k,1,new QTableWidgetItem(QString::number(k)));
    }
    for (int i = 0; i < w2.kns.size(); ++i) {
          ss = w2.kns[i];
          ui->comboBox_6->addItem(ss[0]);
         }
    w.ss=w2.kns[0];
    objectProgramPhase = ObjectProgramIdle;
    objectProgramFrameIndex = 0;
    objectProgramPollCount = 0;
    objectProgramOffset = 0;
    objectCatalogIndex = 0;
    objectProgramCancelled = false;
    floatConfigBusy = false;
    floatConfigWriting = false;
    objectProgramTimeoutTimer = new QTimer(this);
    objectProgramTimeoutTimer->setSingleShot(true);
    connect(objectProgramTimeoutTimer, SIGNAL(timeout()),
            this, SLOT(objectProgramTimeout()));
    setupObjectProgrammingPage(!ports.isEmpty());
    setupProfilerPage();
    setupSlot1Page();
    {
        const int oldFloatTab =
            ui->tabWidget->indexOf(ui->tab_float_config);
        if (oldFloatTab >= 0)
            ui->tabWidget->removeTab(oldFloatTab);
    }
    setupTagConfigPage();
    setupLuaPage();
    ui->tabWidget->setCurrentWidget(objectProgramTab);
}

ex::~ex()
{
    delete ui;
}

void ex::setupLuaPage()
{
    luaRunAfterWrite = false;
    luaComWriteActive = false;
    luaComReadActive = false;
    luaRuntimeActive = false;
    luaTagsRefreshActive = false;
    luaRefreshTagsAfterRead = false;
    luaTagsValuesTimer = new QTimer(this);
    luaTagsValuesTimer->setSingleShot(true);
    connect(luaTagsValuesTimer, SIGNAL(timeout()),
            this, SLOT(luaTagsValuesTimeout()));
    luaTagsAutoTimer = new QTimer(this);
    luaTagsAutoTimer->setSingleShot(true);
    connect(luaTagsAutoTimer, SIGNAL(timeout()),
            this, SLOT(luaTagsAutoRefresh()));
    luaSlotsStatusTimer = new QTimer(this);
    luaSlotsStatusTimer->setSingleShot(true);
    connect(luaSlotsStatusTimer, SIGNAL(timeout()),
            this, SLOT(luaSlotsStatusTimeout()));
    luaTab = new QWidget(ui->tabWidget);
    QVBoxLayout *page = new QVBoxLayout(luaTab);

    QGridLayout *settings = new QGridLayout;
    settings->addWidget(new QLabel(QString::fromUtf8("Контроллер:"), luaTab), 0, 0);
    luaHostEdit = new QLineEdit("192.168.1.100", luaTab);
    luaHostEdit->setToolTip(QString::fromUtf8("IP-адрес или http://IP:порт"));
    settings->addWidget(luaHostEdit, 0, 1);
    settings->addWidget(new QLabel(QString::fromUtf8("Object ID:"), luaTab), 0, 2);
    luaObjectIdSpin = new QSpinBox(luaTab);
    luaObjectIdSpin->setRange(1, 0x7fffffff);
    luaObjectIdSpin->setValue(4);
    settings->addWidget(luaObjectIdSpin, 0, 3);
    settings->addWidget(new QLabel(QString::fromUtf8("Имя:"), luaTab), 0, 4);
    luaObjectNameEdit = new QLineEdit("web.lua", luaTab);
    luaObjectNameEdit->setMaxLength(39);
    settings->addWidget(luaObjectNameEdit, 0, 5);
    settings->addWidget(new QLabel(QString::fromUtf8("Lua-слот COM:"), luaTab), 1, 0);
    luaComSlotSpin = new QSpinBox(luaTab);
    luaComSlotSpin->setRange(1, 32);
    luaComSlotSpin->setValue(1);
    settings->addWidget(luaComSlotSpin, 1, 1);
    QLabel *luaComHint = new QLabel(
        QString::fromUtf8("vm_01.lua, пул блоков 128..159, максимум 3968 байт"), luaTab);
    luaComHint->setObjectName("luaComHint");
    settings->addWidget(luaComHint, 1, 2, 1, 2);
    settings->addWidget(new QLabel("VM ID:", luaTab), 1, 4);
    luaVmIdSpin = new QSpinBox(luaTab);
    luaVmIdSpin->setRange(1, 0x7fffffff);
    luaVmIdSpin->setValue(5);
    settings->addWidget(luaVmIdSpin, 1, 5);
    page->addLayout(settings);

    QHBoxLayout *buttons = new QHBoxLayout;
    luaReadButton = new QPushButton(QString::fromUtf8("Прочитать"), luaTab);
    luaWriteButton = new QPushButton(QString::fromUtf8("Записать"), luaTab);
    luaComWriteButton = new QPushButton(QString::fromUtf8("COM-запись"), luaTab);
    luaComReadButton = new QPushButton(QString::fromUtf8("COM-чтение"), luaTab);
    luaRunButton = new QPushButton(QString::fromUtf8("Запустить VM"), luaTab);
    luaStopButton = new QPushButton(QString::fromUtf8("Остановить VM"), luaTab);
    luaWriteRunButton = new QPushButton(QString::fromUtf8("Записать и запустить"), luaTab);
    luaStatusButton = new QPushButton(QString::fromUtf8("Статус VM"), luaTab);
    luaSlotsStatusButton = new QPushButton(QString::fromUtf8("Статус слотов"), luaTab);
    luaTagsButton = new QPushButton(QString::fromUtf8("Обновить теги"), luaTab);
    buttons->addWidget(luaReadButton);
    buttons->addWidget(luaWriteButton);
    buttons->addWidget(luaComWriteButton);
    buttons->addWidget(luaComReadButton);
    buttons->addWidget(luaRunButton);
    buttons->addWidget(luaStopButton);
    buttons->addWidget(luaWriteRunButton);
    buttons->addWidget(luaStatusButton);
    buttons->addWidget(luaSlotsStatusButton);
    buttons->addWidget(luaTagsButton);
    buttons->addStretch(1);
    page->addLayout(buttons);

    luaSourceEdit = new QTextEdit(luaTab);
    luaSourceEdit->setAcceptRichText(false);
    luaSourceEdit->setFontFamily("Consolas");
    luaSourceEdit->setPlainText(
        "-- period_ms=1000\n"
        "local value, valid = tag.get(2, 1, 1)\n\n"
        "if value ~= nil and valid then\n"
        "  local ok, err = tag.set(1, 1, 2, value)\n"
        "end\n");
    page->addWidget(luaSourceEdit, 2);

    luaStatusLabel = new QLabel(QString::fromUtf8("Готово"), luaTab);
    page->addWidget(luaStatusLabel);
    luaSlotsTable = new QTableWidget(luaTab);
    luaSlotsTable->setColumnCount(4);
    luaSlotsTable->setRowCount(32);
    luaSlotsTable->setHorizontalHeaderLabels(QStringList()
        << QString::fromUtf8("Слот") << QString::fromUtf8("Объект")
        << QString::fromUtf8("Состояние") << QString::fromUtf8("Результат"));
    luaSlotsTable->setEditTriggers(QAbstractItemView::NoEditTriggers);
    luaSlotsTable->verticalHeader()->setVisible(false);
    luaSlotsTable->horizontalHeader()->setResizeMode(2, QHeaderView::Stretch);
    luaSlotsTable->setWindowFlags(Qt::Window);
    luaSlotsTable->setWindowTitle(QString::fromUtf8("Состояние Lua-слотов"));
    luaSlotsTable->resize(620, 500);
    for (int slot = 1; slot <= 32; ++slot)
    {
        luaSlotsTable->setItem(slot - 1, 0,
            new QTableWidgetItem(QString::number(slot)));
        luaSlotsTable->setItem(slot - 1, 1,
            new QTableWidgetItem(QString("vm_%1.lua").arg(slot, 2, 10, QChar('0'))));
        luaSlotsTable->setItem(slot - 1, 2,
            new QTableWidgetItem(QString::fromUtf8("нет")));
        luaSlotsTable->setItem(slot - 1, 3, new QTableWidgetItem("0"));
    }
    luaTagsTable = new QTableWidget(luaTab);
    luaTagsTable->setColumnCount(7);
    luaTagsTable->setHorizontalHeaderLabels(QStringList()
        << QString::fromUtf8("Порт") << QString::fromUtf8("Устройство")
        << "ID" << QString::fromUtf8("Имя") << QString::fromUtf8("Тип")
        << QString::fromUtf8("Значение") << QString::fromUtf8("Флаги"));
    luaTagsTable->setEditTriggers(QAbstractItemView::NoEditTriggers);
    luaTagsTable->setSelectionBehavior(QAbstractItemView::SelectRows);
    luaTagsTable->verticalHeader()->setVisible(false);
    luaTagsTable->horizontalHeader()->setResizeMode(3, QHeaderView::Stretch);
    page->addWidget(luaTagsTable, 1);

    luaNetwork = new QNetworkAccessManager(this);
    connect(luaReadButton, SIGNAL(clicked()), this, SLOT(luaRead()));
    connect(luaWriteButton, SIGNAL(clicked()), this, SLOT(luaWrite()));
    connect(luaComWriteButton, SIGNAL(clicked()), this, SLOT(luaComWrite()));
    connect(luaComReadButton, SIGNAL(clicked()), this, SLOT(luaComRead()));
    connect(luaComSlotSpin, SIGNAL(valueChanged(int)),
            this, SLOT(luaSlotChanged(int)));
    connect(luaRunButton, SIGNAL(clicked()), this, SLOT(luaRun()));
    connect(luaStopButton, SIGNAL(clicked()), this, SLOT(luaStop()));
    connect(luaWriteRunButton, SIGNAL(clicked()), this, SLOT(luaWriteAndRun()));
    connect(luaStatusButton, SIGNAL(clicked()), this, SLOT(luaStatus()));
    connect(luaSlotsStatusButton, SIGNAL(clicked()), this, SLOT(luaSlotsStatus()));
    connect(luaTagsButton, SIGNAL(clicked()), this, SLOT(luaRefreshTags()));
    connect(luaNetwork, SIGNAL(finished(QNetworkReply*)),
            this, SLOT(luaReplyFinished(QNetworkReply*)));
    connect(luaTagsTable, SIGNAL(cellDoubleClicked(int,int)),
            this, SLOT(luaInsertTag(int,int)));
    luaSlotChanged(luaComSlotSpin->value());
    ui->tabWidget->addTab(luaTab, "Lua");
}

void ex::luaSetBusy(bool busy)
{
    luaReadButton->setEnabled(!busy);
    luaWriteButton->setEnabled(!busy);
    luaComWriteButton->setEnabled(!busy);
    luaComReadButton->setEnabled(!busy);
    luaComSlotSpin->setEnabled(!busy);
    luaVmIdSpin->setEnabled(!busy);
    luaRunButton->setEnabled(!busy);
    luaStopButton->setEnabled(!busy);
    luaWriteRunButton->setEnabled(!busy);
    luaStatusButton->setEnabled(!busy);
    luaSlotsStatusButton->setEnabled(!busy);
    luaTagsButton->setEnabled(!busy);
}

void ex::luaShowStatus(const QString &message, bool success)
{
    luaStatusLabel->setText(message);
    luaStatusLabel->setStyleSheet(success ? "color:#087830" : "color:#b02020");
}

void ex::luaStopTagRefresh()
{
    luaTagsAutoTimer->stop();
    luaTagsValuesTimer->stop();
    disconnect(&w, SIGNAL(s_rd()), this, SLOT(luaTagsValuesResponse()));
    luaTagsRefreshActive = false;
}

void ex::luaRequest(const QString &operation)
{
    QString host = luaHostEdit->text().trimmed();
    if (host.isEmpty())
    {
        luaShowStatus(QString::fromUtf8("Укажите адрес контроллера"), false);
        return;
    }
    if (!host.startsWith("http://", Qt::CaseInsensitive) &&
        !host.startsWith("https://", Qt::CaseInsensitive))
        host.prepend("http://");
    while (host.endsWith('/'))
        host.chop(1);

    QString path;
    if (operation == "read" || operation == "write")
    {
        path = QString("/api/lua/script?id=%1&name=%2")
            .arg(luaObjectIdSpin->value())
            .arg(QString::fromLatin1(QUrl::toPercentEncoding(
                luaObjectNameEdit->text())));
    }
    else if (operation == "run")
        path = "/api/lua/run";
    else if (operation == "status")
        path = "/api/lua/status";
    else
        path = "/api/tags";

    luaPendingOperation = operation;
    luaSetBusy(true);
    luaShowStatus(QString::fromUtf8("Обмен с контроллером…"), true);
    QNetworkRequest request(QUrl(host + path));
    request.setRawHeader("Cache-Control", "no-store");
    if (operation == "write")
    {
        request.setHeader(QNetworkRequest::ContentTypeHeader,
                          "text/plain; charset=utf-8");
        luaNetwork->put(request, luaSourceEdit->toPlainText().toUtf8());
    }
    else if (operation == "run")
        luaNetwork->post(request, QByteArray());
    else
        luaNetwork->get(request);
}

void ex::luaRead() { luaRunAfterWrite = false; luaComRead(); }
void ex::luaWrite() { luaRunAfterWrite = false; luaComWrite(); }

void ex::luaSlotChanged(int slot)
{
    const quint32 objectId = 0x4C530000U + quint32(slot);
    const QString name = QString("vm_%1.lua").arg(slot, 2, 10, QChar('0'));
    luaObjectIdSpin->setValue(int(objectId));
    luaObjectNameEdit->setText(name);
    QLabel *hint = luaTab->findChild<QLabel *>("luaComHint");
    if (hint)
        hint->setText(QString::fromUtf8(
            "%1, пул блоков 128..159, максимум 3968 байт").arg(name));
}

void ex::luaComWrite()
{
    luaStopTagRefresh();
    const QByteArray payload = luaSourceEdit->toPlainText().toUtf8();
    if (payload.isEmpty())
    {
        luaShowStatus(QString::fromUtf8("Lua-скрипт пуст"), false);
        return;
    }
    if (payload.size() > 3968)
    {
        luaShowStatus(QString::fromUtf8(
            "Скрипт %1 байт; максимум для COM-слота 3968")
            .arg(payload.size()), false);
        return;
    }
    if (objectProgramPhase != ObjectProgramIdle)
    {
        luaShowStatus(QString::fromUtf8("Другая OBJ1-операция уже выполняется"), false);
        return;
    }
    if (!w.com_S && (!w.port || !w.port->isOpen()))
    {
        luaShowStatus(QString::fromUtf8("COM6 не открыт"), false);
        return;
    }

    const int slot = luaComSlotSpin->value();
    const quint32 objectId = 0x4C530000U + quint32(slot);
    const QString name = QString("vm_%1.lua").arg(slot, 2, 10, QChar('0'));
    const int typeIndex = objectTypeCombo->findData(
        ObjectProgramming::ObjectLuaScript);
    if (typeIndex >= 0)
        objectTypeCombo->setCurrentIndex(typeIndex);
    objectIdSpin->setValue(int(objectId));
    objectNameEdit->setText(name);
    objectAutostartCheck->setChecked(false);
    objectReadonlyCheck->setChecked(true);
    objectSystemCheck->setChecked(true);
    objectCompressedCheck->setChecked(false);
    objectProgramPayload = payload;
    objectProgramImage.clear();
    objectFileEdit->setText(QString::fromUtf8("Редактор Lua: %1").arg(name));
    objectWriteButton->setEnabled(true);

    luaComWriteActive = true;
    luaSetBusy(true);
    luaShowStatus(QString::fromUtf8("COM6: запись %1…").arg(name), true);
    objectProgramWrite();
}

void ex::luaComRead()
{
    luaStopTagRefresh();
    if (objectProgramPhase != ObjectProgramIdle)
    {
        luaShowStatus(QString::fromUtf8("Другая OBJ1-операция уже выполняется"), false);
        return;
    }
    if (!w.com_S && (!w.port || !w.port->isOpen()))
    {
        luaShowStatus(QString::fromUtf8("COM6 не открыт"), false);
        return;
    }
    const quint32 objectId = 0x4C530000U +
        quint32(luaComSlotSpin->value());
    luaComReadPayload.clear();
    luaComReadExpected = 0;
    luaComReadOffset = 0;
    luaComReadActive = true;
    luaSetBusy(true);
    luaShowStatus(QString::fromUtf8("COM6: чтение %1…")
                  .arg(luaObjectNameEdit->text()), true);
    disconnect(&w, SIGNAL(s_rd()), 0, 0);
    connect(&w, SIGNAL(s_rd()), this, SLOT(objectProgramResponse()));
    objectProgramPhase = ObjectProgramLuaReadSelect;
    objectProgramPollCount = 0;
    objectProgramSend(ObjectProgramming::makeObjectSelectFrame(
        objectId, w.mb_rtu));
}

void ex::luaRun()
{
    luaStopTagRefresh();
    luaRunAfterWrite = false;
    objectIdSpin->setValue(luaVmIdSpin->value());
    luaRuntimeActive = true;
    luaSetBusy(true);
    luaShowStatus(QString::fromUtf8("UDP: запуск Lua VM id=%1...")
                  .arg(luaVmIdSpin->value()), true);
    objectProgramRuntime(ObjectProgramStartSelect,
        QString::fromUtf8("Запуск Lua VM id=%1...").arg(luaVmIdSpin->value()));
}

void ex::luaStop()
{
    luaStopTagRefresh();
    luaRunAfterWrite = false;
    objectIdSpin->setValue(luaVmIdSpin->value());
    luaRuntimeActive = true;
    luaSetBusy(true);
    luaShowStatus(QString::fromUtf8("UDP: остановка Lua VM id=%1...")
                  .arg(luaVmIdSpin->value()), true);
    objectProgramRuntime(ObjectProgramStopSelect,
        QString::fromUtf8("Остановка Lua VM id=%1...").arg(luaVmIdSpin->value()));
}

void ex::luaStatus()
{
    luaStopTagRefresh();
    luaRunAfterWrite = false;
    objectIdSpin->setValue(luaVmIdSpin->value());
    luaRuntimeActive = true;
    luaSetBusy(true);
    luaShowStatus(QString::fromUtf8("UDP: статус Lua VM id=%1...")
                  .arg(luaVmIdSpin->value()), true);
    objectProgramRuntime(ObjectProgramStatusSelect,
        QString::fromUtf8("Статус Lua VM id=%1...").arg(luaVmIdSpin->value()));
}

void ex::luaSlotsStatus()
{
    luaStopTagRefresh();
    luaSlotsTable->show();
    luaSlotsTable->raise();
    luaSlotsTable->activateWindow();
    if (objectProgramPhase != ObjectProgramIdle)
    {
        luaShowStatus(QString::fromUtf8("Другая OBJ1-операция уже выполняется"), false);
        return;
    }
    if (!w.com_S && (!w.port || !w.port->isOpen()))
    {
        luaShowStatus(QString::fromUtf8("COM-порт не открыт"), false);
        return;
    }
    luaSetBusy(true);
    disconnect(&w, SIGNAL(s_rd()), 0, 0);
    connect(&w, SIGNAL(s_rd()), this, SLOT(luaSlotsStatusResponse()));
    luaShowStatus(QString::fromUtf8("UDP: чтение состояния 32 Lua-слотов..."), true);
    if (!w.rd_reg(40001U + 32000U, 100U))
    {
        luaSlotsStatusTimeout();
        return;
    }
    luaSlotsStatusTimer->start(2000);
}

void ex::luaSlotsStatusResponse()
{
    luaSlotsStatusTimer->stop();
    disconnect(&w, SIGNAL(s_rd()), this, SLOT(luaSlotsStatusResponse()));
    const int functionOffset = 1 + w.ElamFlag;
    const int dataOffset = w.ElamFlag ? functionOffset + 3 : functionOffset + 2;
    const int receivedBytes = w.ElamFlag ?
        (((int)w.buf_in[functionOffset + 1] << 8) |
         w.buf_in[functionOffset + 2]) : w.buf_in[functionOffset + 1];
    if (!w.CRC_ok || w.buf_in[functionOffset] != 3 || receivedBytes != 200)
    {
        luaSlotsStatusTimeout();
        return;
    }
    QVector<quint16> words;
    words.reserve(100);
    for (int index = 0; index < 100; ++index)
        words.append((quint16)w.get_word1(dataOffset + 1 + index * 2));
    if (words.at(0) != 1U)
    {
        luaSetBusy(false);
        luaShowStatus(QString::fromUtf8("Неизвестная версия статуса Lua-слотов"), false);
        return;
    }
    const quint32 mask = (quint32(words.at(2)) << 16) | words.at(3);
    QStringList activeSlots;
    for (int slot = 0; slot < 32; ++slot)
    {
        const quint16 state = words.at(4 + slot * 3);
        const qint32 result = qint32((quint32(words.at(5 + slot * 3)) << 16) |
                                    words.at(6 + slot * 3));
        QString stateText;
        if (state == 1U) stateText = QString::fromUtf8("работает");
        else if (state == 2U) stateText = QString::fromUtf8("выполнен");
        else if (state == 3U) stateText = QString::fromUtf8("ошибка");
        else stateText = QString::fromUtf8("нет");
        luaSlotsTable->item(slot, 2)->setText(stateText);
        luaSlotsTable->item(slot, 3)->setText(QString::number(result));
        if (mask & (1UL << slot))
            activeSlots << QString::number(slot + 1);
    }
    luaSetBusy(false);
    luaShowStatus(QString::fromUtf8("Работает Lua-слотов: %1 из 32%2")
        .arg(words.at(1))
        .arg(activeSlots.isEmpty() ? QString() :
             QString::fromUtf8(" — ") + activeSlots.join(", ")), true);
}

void ex::luaSlotsStatusTimeout()
{
    luaSlotsStatusTimer->stop();
    disconnect(&w, SIGNAL(s_rd()), this, SLOT(luaSlotsStatusResponse()));
    luaSetBusy(false);
    luaShowStatus(QString::fromUtf8(
        "Нет ответа на UDP-запрос состояния Lua-слотов"), false);
}

void ex::luaRefreshTags()
{
    luaRunAfterWrite = false;
    luaTagsAutoTimer->stop();
    if (!luaRefreshTagsAfterRead)
    {
        luaRefreshTagsAfterRead = true;
        luaComRead();
        return;
    }
    luaRefreshTagsAfterRead = false;
    luaTagKeys.clear();
    luaTagKeyIndex = 0;
    QRegExp reference(
        "tag\\.(get|set)\\s*\\(\\s*([0-9]+)\\s*,\\s*([0-9]+)\\s*,\\s*([0-9]+)");
    const QString source = luaSourceEdit->toPlainText();
    int position = 0;
    while ((position = reference.indexIn(source, position)) >= 0)
    {
        const int port = reference.cap(2).toInt();
        const int device = reference.cap(3).toInt();
        const int sensor = reference.cap(4).toInt();
        position += qMax(1, reference.matchedLength());
        if (port < 1 || port > 5 || device < 1 || device > 30 ||
            sensor < 1 || sensor > 30)
            continue;
        const quint32 key = (quint32(port) << 16) |
                            (quint32(device) << 8) | quint32(sensor);
        if (!luaTagKeys.contains(key))
            luaTagKeys.append(key);
    }
    if (luaTagKeys.isEmpty())
    {
        luaShowStatus(QString::fromUtf8(
            "В тексте нет вызовов tag.get/tag.set"), false);
        return;
    }
    luaTagsRefreshActive = true;
    luaSetBusy(true);
    luaTagsTable->setRowCount(luaTagKeys.size());
    for (int row = 0; row < luaTagKeys.size(); ++row)
    {
        const quint32 key = luaTagKeys.at(row);
        const int port = int((key >> 16) & 0xffU);
        const int device = int((key >> 8) & 0xffU);
        const int sensor = int(key & 0xffU);
        luaTagsTable->setItem(row, 0, new QTableWidgetItem(QString::number(port)));
        luaTagsTable->setItem(row, 1, new QTableWidgetItem(QString::number(device)));
        luaTagsTable->setItem(row, 2, new QTableWidgetItem(QString::number(sensor)));
        luaTagsTable->setItem(row, 3, new QTableWidgetItem(
            QString("tag_%1_%2_%3").arg(port).arg(device).arg(sensor)));
        luaTagsTable->setItem(row, 4, new QTableWidgetItem("-"));
        luaTagsTable->setItem(row, 5, new QTableWidgetItem("-"));
        luaTagsTable->setItem(row, 6, new QTableWidgetItem("-"));
    }
    disconnect(&w, SIGNAL(s_rd()), 0, 0);
    connect(&w, SIGNAL(s_rd()), this, SLOT(luaTagsValuesResponse()));
    luaShowStatus(QString::fromUtf8("UDP: чтение %1 тегов из Lua-скрипта...")
                  .arg(luaTagKeys.size()), true);
    luaStartNextTagValueRead();
}

void ex::luaStartNextTagValueRead()
{
    if (luaTagKeyIndex >= luaTagKeys.size())
    {
        luaTagsRefreshActive = false;
        luaSetBusy(false);
        disconnect(&w, SIGNAL(s_rd()), this, SLOT(luaTagsValuesResponse()));
        luaShowStatus(QString::fromUtf8("UDP: обновлено тегов: %1")
                      .arg(luaTagKeys.size()), true);
        luaTagsAutoTimer->start(1000);
        return;
    }
    const quint32 key = luaTagKeys.at(luaTagKeyIndex);
    const unsigned int port = ((key >> 16) & 0xffU) - 1U;
    const unsigned int device = (key >> 8) & 0xffU;
    const unsigned int sensor = key & 0xffU;
    const unsigned int base = 14000U +
        (port * 30U * 30U + (device - 1U) * 30U + sensor - 1U) * 4U;
    if (!w.rd_reg(40001U + base, 4U))
    {
        luaTagsValuesTimeout();
        return;
    }
    luaTagsValuesTimer->start(2000);
}

void ex::luaTagsAutoRefresh()
{
    if (luaTagKeys.isEmpty())
        return;
    if (objectProgramPhase != ObjectProgramIdle || luaTagsRefreshActive)
    {
        luaTagsAutoTimer->start(1000);
        return;
    }
    if (!w.com_S && (!w.port || !w.port->isOpen()))
        return;
    luaTagKeyIndex = 0;
    luaTagsRefreshActive = true;
    disconnect(&w, SIGNAL(s_rd()), 0, 0);
    connect(&w, SIGNAL(s_rd()), this, SLOT(luaTagsValuesResponse()));
    luaStartNextTagValueRead();
}

void ex::luaWriteAndRun()
{
    luaRunAfterWrite = true;
    luaComWrite();
}

void ex::luaReplyFinished(QNetworkReply *reply)
{
    const QString operation = luaPendingOperation;
    const QByteArray body = reply->readAll();
    const bool networkOk = reply->error() == QNetworkReply::NoError;
    reply->deleteLater();

    if (!networkOk)
    {
        luaRunAfterWrite = false;
        luaSetBusy(false);
        luaShowStatus(QString::fromUtf8("Ошибка сети: ") + reply->errorString(), false);
        return;
    }

    if (operation == "read")
    {
        luaSourceEdit->setPlainText(QString::fromUtf8(body));
        luaShowStatus(QString::fromUtf8("Скрипт прочитан"), true);
    }
    else if (operation == "write")
    {
        const bool ok = body.contains("\"result\":0");
        QRegExp generation("\\\"generation\\\":([0-9]+)");
        generation.indexIn(QString::fromLatin1(body));
        if (!ok)
        {
            luaRunAfterWrite = false;
            luaSetBusy(false);
            luaShowStatus(QString::fromUtf8("Запись не выполнена: ") +
                          QString::fromUtf8(body), false);
            return;
        }
        if (luaRunAfterWrite)
        {
            luaRunAfterWrite = false;
            luaSetBusy(false);
            luaRequest("run");
            return;
        }
        luaShowStatus(QString::fromUtf8("Скрипт записан, поколение ") +
                      generation.cap(1), true);
    }
    else if (operation == "run")
    {
        const bool ok = body.contains("\"result\":0");
        luaShowStatus(ok ? QString::fromUtf8("Lua VM запущена") :
                      QString::fromUtf8("Ошибка запуска: ") + QString::fromUtf8(body), ok);
        if (ok)
            QTimer::singleShot(350, this, SLOT(luaStatus()));
    }
    else if (operation == "status")
    {
        const QString text = QString::fromLatin1(body);
        QRegExp active("\\\"active\\\":([0-9]+)");
        QRegExp result("\\\"last_result\\\":(-?[0-9]+)");
        active.indexIn(text);
        result.indexIn(text);
        const bool ok = result.cap(1) == "0";
        luaShowStatus(QString::fromUtf8("VM: ") +
            (active.cap(1) == "1" ? QString::fromUtf8("работает") :
                                    QString::fromUtf8("завершена")) +
            QString::fromUtf8(", результат ") + result.cap(1), ok);
    }
    else if (operation == "tags")
    {
        const QString text = QString::fromUtf8(body);
        QRegExp item("\\{\\\"key\\\":[^\\}]+\\}");
        int pos = 0;
        int row = 0;
        luaTagsTable->setRowCount(0);
        while ((pos = item.indexIn(text, pos)) >= 0)
        {
            const QString object = item.cap(0);
            pos += item.matchedLength();
            QRegExp port("\\\"port\\\":([0-9]+)");
            QRegExp device("\\\"device\\\":([0-9]+)");
            QRegExp id("\\\"id\\\":([0-9]+)");
            QRegExp name("\\\"name\\\":\\\"([^\\\"]*)\\\"");
            QRegExp type("\\\"type\\\":([0-9]+)");
            QRegExp flags("\\\"flags\\\":([0-9]+)");
            QRegExp bits("\\\"value_bits\\\":([0-9]+)");
            port.indexIn(object); device.indexIn(object); id.indexIn(object);
            name.indexIn(object); type.indexIn(object); flags.indexIn(object);
            bits.indexIn(object);
            const int typeValue = type.cap(1).toInt();
            const quint32 raw = bits.cap(1).toUInt();
            QString value;
            if (typeValue == 0)
            {
                float number;
                memcpy(&number, &raw, sizeof(number));
                value = QString::number(number, 'g', 9);
            }
            else if (typeValue == 1)
                value = raw ? "true" : "false";
            else if (typeValue == 3)
                value = QString::number((qint16)(raw & 0xffffU));
            else
                value = QString::number(raw);
            static const char *typeNames[] = {"float32", "bool", "uint16", "int16", "uint32", "int32"};
            luaTagsTable->insertRow(row);
            luaTagsTable->setItem(row, 0, new QTableWidgetItem(port.cap(1)));
            luaTagsTable->setItem(row, 1, new QTableWidgetItem(device.cap(1)));
            luaTagsTable->setItem(row, 2, new QTableWidgetItem(id.cap(1)));
            luaTagsTable->setItem(row, 3, new QTableWidgetItem(name.cap(1)));
            luaTagsTable->setItem(row, 4, new QTableWidgetItem(
                typeValue >= 0 && typeValue < 6 ? typeNames[typeValue] : type.cap(1)));
            luaTagsTable->setItem(row, 5, new QTableWidgetItem(value));
            luaTagsTable->setItem(row, 6, new QTableWidgetItem(flags.cap(1)));
            ++row;
        }
        luaShowStatus(QString::fromUtf8("Теги обновлены: ") + QString::number(row), true);
    }
    luaSetBusy(false);
}

void ex::luaInsertTag(int row, int)
{
    if (row < 0 || !luaTagsTable->item(row, 0) ||
        !luaTagsTable->item(row, 1) || !luaTagsTable->item(row, 2))
        return;
    const QString tuple = luaTagsTable->item(row, 0)->text() + ", " +
                          luaTagsTable->item(row, 1)->text() + ", " +
                          luaTagsTable->item(row, 2)->text();
    luaSourceEdit->insertPlainText(tuple);
    luaSourceEdit->setFocus();
}

void ex::setupObjectProgrammingPage(bool portAvailable)
{
    objectProgramTab = ui->tab_obj1;
    QVBoxLayout *pageLayout = new QVBoxLayout(objectProgramTab);
    QGroupBox *group = new QGroupBox(
        tr("Универсальный загрузчик OBJ1 через ELAM / COM"), objectProgramTab);
    QVBoxLayout *groupLayout = new QVBoxLayout(group);

    QHBoxLayout *fileLayout = new QHBoxLayout;
    fileLayout->addWidget(new QLabel(tr("Payload:"), group));
    objectFileEdit = new QLineEdit(group);
    objectFileEdit->setReadOnly(true);
    fileLayout->addWidget(objectFileEdit, 1);
    objectOpenButton = new QPushButton(tr("Открыть"), group);
    objectOpenButton->setEnabled(portAvailable);
    fileLayout->addWidget(objectOpenButton);
    groupLayout->addLayout(fileLayout);

    QGridLayout *settings = new QGridLayout;
    settings->addWidget(new QLabel(tr("Тип OBJ1:"), group), 0, 0);
    objectTypeCombo = new QComboBox(group);
    objectTypeCombo->addItem(tr("XIP module"),
                             ObjectProgramming::ObjectXipModule);
    objectTypeCombo->addItem(tr("Web file"),
                             ObjectProgramming::ObjectWebFile);
    objectTypeCombo->addItem(tr("Lua VM"),
                             ObjectProgramming::ObjectLuaVm);
    objectTypeCombo->addItem(tr("Lua script"),
                             ObjectProgramming::ObjectLuaScript);
    objectTypeCombo->addItem(tr("Bytecode"),
                             ObjectProgramming::ObjectBytecode);
    objectTypeCombo->addItem(tr("Device profile"),
                             ObjectProgramming::ObjectDeviceProfile);
    objectTypeCombo->addItem(tr("Configuration"),
                             ObjectProgramming::ObjectConfiguration);
    objectTypeCombo->addItem(tr("Tag dictionary"),
                             ObjectProgramming::ObjectTagDictionary);
    settings->addWidget(objectTypeCombo, 0, 1);

    settings->addWidget(new QLabel(tr("Object ID:"), group), 0, 2);
    objectIdSpin = new QSpinBox(group);
    objectIdSpin->setRange(1, 0x7FFFFFFF);
    objectIdSpin->setValue(1);
    settings->addWidget(objectIdSpin, 0, 3);

    settings->addWidget(new QLabel(tr("Имя / URL:"), group), 1, 0);
    objectNameEdit = new QLineEdit(group);
    objectNameEdit->setMaxLength(39);
    settings->addWidget(objectNameEdit, 1, 1, 1, 3);

    objectContentTypeLabel = new QLabel(tr("Content type:"), group);
    settings->addWidget(objectContentTypeLabel, 2, 0);
    objectContentTypeCombo = new QComboBox(group);
    objectContentTypeCombo->addItems(QStringList()
        << "HTML" << "CSS" << "JavaScript" << "JSON" << "PNG"
        << "JPEG" << "SVG" << "Text" << "ICO");
    settings->addWidget(objectContentTypeCombo, 2, 1);

    objectApiVersionLabel = new QLabel(tr("API version:"), group);
    settings->addWidget(objectApiVersionLabel, 2, 2);
    objectApiVersionSpin = new QSpinBox(group);
    objectApiVersionSpin->setRange(0, 65535);
    objectApiVersionSpin->setValue(5);
    settings->addWidget(objectApiVersionSpin, 2, 3);

    objectLinkAddressLabel = new QLabel(tr("Link address:"), group);
    settings->addWidget(objectLinkAddressLabel, 3, 0);
    objectLinkAddressEdit = new QLineEdit("0x90004080", group);
    settings->addWidget(objectLinkAddressEdit, 3, 1);
    objectEntryOffsetLabel = new QLabel(tr("Entry offset:"), group);
    settings->addWidget(objectEntryOffsetLabel, 3, 2);
    objectEntryOffsetSpin = new QSpinBox(group);
    objectEntryOffsetSpin->setRange(0, 0x7FFFFFFF);
    settings->addWidget(objectEntryOffsetSpin, 3, 3);
    groupLayout->addLayout(settings);

    QHBoxLayout *flagsLayout = new QHBoxLayout;
    objectAutostartCheck = new QCheckBox(tr("AUTOSTART"), group);
    objectReadonlyCheck = new QCheckBox(tr("READONLY"), group);
    objectSystemCheck = new QCheckBox(tr("SYSTEM"), group);
    objectCompressedCheck = new QCheckBox(tr("COMPRESSED"), group);
    flagsLayout->addWidget(objectAutostartCheck);
    flagsLayout->addWidget(objectReadonlyCheck);
    flagsLayout->addWidget(objectSystemCheck);
    flagsLayout->addWidget(objectCompressedCheck);
    flagsLayout->addStretch(1);
    groupLayout->addLayout(flagsLayout);

    QHBoxLayout *buttonLayout = new QHBoxLayout;
    objectWriteButton = new QPushButton(tr("Записать OBJ1"), group);
    objectWriteButton->setEnabled(false);
    objectCancelButton = new QPushButton(tr("Отмена"), group);
    objectCancelButton->setEnabled(false);
    objectStatusButton = new QPushButton(QString::fromUtf8("Статус"), group);
    objectStatusButton->setEnabled(portAvailable);
    objectStartButton = new QPushButton(QString::fromUtf8("Запустить"), group);
    objectStartButton->setEnabled(portAvailable);
    objectStopButton = new QPushButton(QString::fromUtf8("Остановить"), group);
    objectStopButton->setEnabled(portAvailable);
    objectCatalogButton = new QPushButton(
        QString::fromUtf8("Прочитать каталог"), group);
    objectCatalogButton->setEnabled(portAvailable);
    buttonLayout->addWidget(objectWriteButton);
    buttonLayout->addWidget(objectCancelButton);
    buttonLayout->addWidget(objectStatusButton);
    buttonLayout->addWidget(objectStartButton);
    buttonLayout->addWidget(objectStopButton);
    buttonLayout->addWidget(objectCatalogButton);
    buttonLayout->addStretch(1);
    groupLayout->addLayout(buttonLayout);

    QHBoxLayout *catalogLayout = new QHBoxLayout;
    catalogLayout->addWidget(new QLabel(
        QString::fromUtf8("Загруженные объекты:"), group));
    objectCatalogCombo = new QComboBox(group);
    catalogLayout->addWidget(objectCatalogCombo, 1);
    groupLayout->addLayout(catalogLayout);

    objectProgress = new QProgressBar(group);
    objectProgress->setValue(0);
    groupLayout->addWidget(objectProgress);
    objectLog = new QTextEdit(group);
    objectLog->setReadOnly(true);
    groupLayout->addWidget(objectLog, 1);
    QLabel *buildLabel = new QLabel(
        tr("OBJ1: header 128 байт, блок 4096 байт, CRC32 header/payload/image"),
        group);
    buildLabel->setAlignment(Qt::AlignRight | Qt::AlignVCenter);
    groupLayout->addWidget(buildLabel);

    pageLayout->addWidget(group);

    connect(objectOpenButton, SIGNAL(clicked()),
            this, SLOT(objectProgramOpen()));
    connect(objectWriteButton, SIGNAL(clicked()),
            this, SLOT(objectProgramWrite()));
    connect(objectCancelButton, SIGNAL(clicked()),
            this, SLOT(objectProgramCancel()));
    connect(objectStatusButton, SIGNAL(clicked()),
            this, SLOT(objectProgramStatus()));
    connect(objectStartButton, SIGNAL(clicked()),
            this, SLOT(objectProgramStart()));
    connect(objectStopButton, SIGNAL(clicked()),
            this, SLOT(objectProgramStop()));
    connect(objectCatalogButton, SIGNAL(clicked()),
            this, SLOT(objectProgramCatalog()));
    connect(objectCatalogCombo, SIGNAL(currentIndexChanged(int)),
            this, SLOT(objectCatalogSelected(int)));
    connect(objectTypeCombo, SIGNAL(currentIndexChanged(int)),
            this, SLOT(objectProgramTypeChanged(int)));
    objectProgramTypeChanged(objectTypeCombo->currentIndex());
}

void ex::setupProfilerPage()
{
    profilerBusy = false;
    profilerEnableActive = false;
    profilerReadingHeader = true;
    profilerThreadCount = 0;
    profilerNextRegister = 0;
    profilerRequestAddress = 0;
    profilerRequestCount = 0;

    profilerTab = ui->tab_profiler;
    profilerStatus = ui->profilerStatus;
    profilerLoad = ui->profilerLoad;
    profilerWindow = ui->profilerWindow;
    profilerAuto = ui->profilerAuto;
    profilerRefreshButton = ui->profilerRefreshButton;
    profilerEnableButton = new QPushButton(QString::fromUtf8("Включить"), profilerTab);
    ui->profilerStatusLayout->insertWidget(
        ui->profilerStatusLayout->count() - 1, profilerEnableButton);
    profilerTable = ui->profilerTable;
    profilerTable->setColumnCount(8);
    QStringList headers;
    headers << QString::fromUtf8("Поток")
            << QString::fromUtf8("CPU, %")
            << QString::fromUtf8("Тики/окно")
            << QString::fromUtf8("Время/окно, мкс")
            << QString::fromUtf8("Всего, мкс")
            << QString::fromUtf8("Макс. запуск, мкс")
            << QString::fromUtf8("Переключений")
            << QString::fromUtf8("Приоритет / состояние");
    profilerTable->setHorizontalHeaderLabels(headers);
    profilerTable->setAlternatingRowColors(true);
    profilerTable->setEditTriggers(QAbstractItemView::NoEditTriggers);
    profilerTable->setSelectionBehavior(QAbstractItemView::SelectRows);
    profilerTable->verticalHeader()->setVisible(false);
    profilerTable->horizontalHeader()->setResizeMode(0, QHeaderView::Stretch);
    for (int column = 1; column < profilerTable->columnCount(); ++column)
        profilerTable->horizontalHeader()->setResizeMode(column, QHeaderView::ResizeToContents);
    profilerPollTimer = new QTimer(this);
    profilerPollTimer->setInterval(2000);
    profilerTimeoutTimer = new QTimer(this);
    profilerTimeoutTimer->setSingleShot(true);
    connect(profilerPollTimer, SIGNAL(timeout()), this, SLOT(profilerPoll()));
    connect(profilerTimeoutTimer, SIGNAL(timeout()), this, SLOT(profilerTimeout()));
    connect(profilerRefreshButton, SIGNAL(clicked()), this, SLOT(profilerRefresh()));
    connect(profilerEnableButton, SIGNAL(clicked()), this, SLOT(profilerEnable()));
    connect(profilerAuto, SIGNAL(toggled(bool)), this, SLOT(profilerAutoChanged(bool)));
    connect(ui->tabWidget, SIGNAL(currentChanged(int)), this, SLOT(profilerTabChanged(int)));
}

void ex::setupSlot1Page()
{
    slot1Tab = ui->tab_slot1_float;
    slot1Status = ui->slot1Status;
    slot1Value = ui->slot1Value;
    slot1Raw = ui->slot1Raw;
    slot1Comm = ui->slot1Comm;
    slot1Success = ui->slot1Success;
    slot1RefreshButton = ui->slot1RefreshButton;
    slot1Auto = ui->slot1Auto;
    slot1Busy = false;

    slot1PollTimer = new QTimer(this);
    slot1PollTimer->setInterval(1000);
    slot1TimeoutTimer = new QTimer(this);
    slot1TimeoutTimer->setSingleShot(true);
    connect(slot1PollTimer, SIGNAL(timeout()), this, SLOT(slot1Poll()));
    connect(slot1TimeoutTimer, SIGNAL(timeout()), this, SLOT(slot1Timeout()));
    connect(slot1RefreshButton, SIGNAL(clicked()), this, SLOT(slot1Refresh()));
    connect(slot1Auto, SIGNAL(toggled(bool)), this, SLOT(slot1AutoChanged(bool)));
    connect(ui->tabWidget, SIGNAL(currentChanged(int)), this, SLOT(slot1TabChanged(int)));
}

void ex::setupFloatConfigPage()
{
    floatConfigTab = ui->tab_float_config;
    floatConfigTable = ui->floatConfigTable;
    floatConfigStatus = ui->floatConfigStatus;
    floatConfigReadButton = ui->floatConfigReadButton;
    floatConfigWriteButton = ui->floatConfigWriteButton;
    floatConfigBusy = false;
    floatConfigWriting = false;

    floatConfigTable->setRowCount(20);
    floatConfigTable->setColumnCount(3);
    QStringList headers;
    headers << QString::fromUtf8("Слот")
            << QString::fromUtf8("Holding")
            << QString::fromUtf8("Порядок float");
    floatConfigTable->setHorizontalHeaderLabels(headers);
    floatConfigTable->verticalHeader()->setVisible(false);
    floatConfigTable->setEditTriggers(QAbstractItemView::NoEditTriggers);
    floatConfigTable->setSelectionBehavior(QAbstractItemView::SelectRows);
    floatConfigTable->setSelectionMode(QAbstractItemView::SingleSelection);
    floatConfigTable->horizontalHeader()->setResizeMode(0, QHeaderView::ResizeToContents);
    floatConfigTable->horizontalHeader()->setResizeMode(1, QHeaderView::ResizeToContents);
    floatConfigTable->horizontalHeader()->setResizeMode(2, QHeaderView::Stretch);

    for (int slot = 0; slot < 20; ++slot)
    {
        QTableWidgetItem *slotItem = new QTableWidgetItem(QString::number(slot));
        QTableWidgetItem *addressItem =
                new QTableWidgetItem(QString::number(2550 + slot));
        slotItem->setTextAlignment(Qt::AlignCenter);
        addressItem->setTextAlignment(Qt::AlignCenter);
        floatConfigTable->setItem(slot, 0, slotItem);
        floatConfigTable->setItem(slot, 1, addressItem);

        QComboBox *order = new QComboBox(floatConfigTable);
        order->addItem("ABCD", 0);
        order->addItem("CDAB", 1);
        order->addItem("BADC", 2);
        order->addItem("DCBA", 3);
        floatConfigTable->setCellWidget(slot, 2, order);
    }
    floatConfigTable->selectRow(1);

    floatConfigTimeoutTimer = new QTimer(this);
    floatConfigTimeoutTimer->setSingleShot(true);
    connect(floatConfigTimeoutTimer, SIGNAL(timeout()),
            this, SLOT(floatConfigTimeout()));
    connect(floatConfigReadButton, SIGNAL(clicked()),
            this, SLOT(floatConfigRead()));
    connect(floatConfigWriteButton, SIGNAL(clicked()),
            this, SLOT(floatConfigWrite()));
}

void ex::floatConfigRead()
{
    if (floatConfigBusy || slot1Busy || profilerBusy ||
        objectProgramPhase != ObjectProgramIdle)
        return;
    if (!w.port->isOpen())
    {
        floatConfigFinish(false, QString::fromUtf8("COM6 не открыт"));
        return;
    }

    floatConfigBusy = true;
    floatConfigWriting = false;
    floatConfigStatus->setText(QString::fromUtf8("Чтение Holding 2550…2569…"));
    disconnect(&w, SIGNAL(s_rd()), 0, 0);
    connect(&w, SIGNAL(s_rd()), this, SLOT(floatConfigResponse()));
    if (!w.rd_reg(42551U, 20U))
    {
        floatConfigFinish(false, QString::fromUtf8("Неверный адрес Modbus"));
        return;
    }
    floatConfigTimeoutTimer->start(1500);
}

void ex::floatConfigWrite()
{
    if (floatConfigBusy || slot1Busy || profilerBusy ||
        objectProgramPhase != ObjectProgramIdle)
        return;
    if (!w.port->isOpen())
    {
        floatConfigFinish(false, QString::fromUtf8("COM6 не открыт"));
        return;
    }

    const int slot = floatConfigTable->currentRow();
    if (slot < 0 || slot >= 20)
    {
        floatConfigFinish(false, QString::fromUtf8("Выберите строку слота"));
        return;
    }
    QComboBox *order =
            qobject_cast<QComboBox *>(floatConfigTable->cellWidget(slot, 2));
    const unsigned int value =
            order ? (unsigned int) order->currentIndex() : 0U;

    floatConfigBusy = true;
    floatConfigWriting = true;
    floatConfigStatus->setText(
        QString::fromUtf8("Запись Holding %1 для слота %2…")
        .arg(2550 + slot).arg(slot));
    disconnect(&w, SIGNAL(s_rd()), 0, 0);
    connect(&w, SIGNAL(s_rd()), this, SLOT(floatConfigResponse()));
    if (!w.wr_reg(42551U + (unsigned int) slot, value))
    {
        floatConfigFinish(false, QString::fromUtf8("Ошибка формирования записи"));
        return;
    }
    floatConfigTimeoutTimer->start(1500);
}

void ex::floatConfigResponse()
{
    floatConfigTimeoutTimer->stop();
    disconnect(&w, SIGNAL(s_rd()), this, SLOT(floatConfigResponse()));

    const int functionOffset = 1 + w.ElamFlag;
    const int expectedFunction = floatConfigWriting ? 6 : 3;
    if (!w.CRC_ok || w.buf_in[functionOffset] != expectedFunction)
    {
        floatConfigFinish(false, QString::fromUtf8("Ошибка ответа или CRC"));
        return;
    }

    if (floatConfigWriting)
    {
        floatConfigFinish(true,
            QString::fromUtf8("Настройка сохранена; перезапустите выбранный слот"));
        return;
    }

    const int dataOffset = w.ElamFlag ? functionOffset + 3 : functionOffset + 2;
    const int expectedBytes = 40;
    const int receivedBytes = w.ElamFlag
            ? (((int) w.buf_in[functionOffset + 1] << 8) |
               w.buf_in[functionOffset + 2])
            : w.buf_in[functionOffset + 1];
    if (w.max_in < (unsigned int) (dataOffset + expectedBytes + 2) ||
        receivedBytes < expectedBytes)
    {
        floatConfigFinish(false, QString::fromUtf8("Короткий ответ Holding"));
        return;
    }

    for (int slot = 0; slot < 20; ++slot)
    {
        const quint16 value =
                (quint16) w.get_word1(dataOffset + 1 + slot * 2);
        QComboBox *order =
                qobject_cast<QComboBox *>(floatConfigTable->cellWidget(slot, 2));
        if (order)
            order->setCurrentIndex(value <= 3U ? value : 0);
    }
    floatConfigFinish(true, QString::fromUtf8("Настройки прочитаны"));
}

void ex::floatConfigTimeout()
{
    disconnect(&w, SIGNAL(s_rd()), this, SLOT(floatConfigResponse()));
    floatConfigFinish(false, QString::fromUtf8("Нет ответа от COM6"));
}

void ex::floatConfigFinish(bool success, const QString &message)
{
    floatConfigTimeoutTimer->stop();
    floatConfigBusy = false;
    floatConfigStatus->setText(message + QString::fromUtf8(" — ") +
                               QDateTime::currentDateTime().toString("hh:mm:ss"));
    floatConfigStatus->setStyleSheet(success ?
                                     "color: #167d2d;" : "color: #b02020;");
}

void ex::slot1TabChanged(int index)
{
    if (ui->tabWidget->widget(index) == slot1Tab)
    {
        if (slot1Auto->isChecked())
            slot1PollTimer->start();
        slot1StartCycle();
    }
    else
    {
        slot1PollTimer->stop();
    }
}

void ex::slot1AutoChanged(bool enabled)
{
    if (enabled && ui->tabWidget->currentWidget() == slot1Tab)
    {
        slot1PollTimer->start();
        slot1StartCycle();
    }
    else
    {
        slot1PollTimer->stop();
    }
}

void ex::slot1Refresh()
{
    slot1StartCycle();
}

void ex::slot1Poll()
{
    if (ui->tabWidget->currentWidget() == slot1Tab)
        slot1StartCycle();
}

void ex::slot1StartCycle()
{
    if (slot1Busy || profilerBusy ||
        objectProgramPhase != ObjectProgramIdle)
        return;
    if (!w.port->isOpen())
    {
        slot1Finish(false, QString::fromUtf8("COM6 не открыт"));
        return;
    }

    slot1Busy = true;
    slot1Status->setText(QString::fromUtf8("Чтение TIT[2512…2523]…"));
    disconnect(&w, SIGNAL(s_rd()), 0, 0);
    connect(&w, SIGNAL(s_rd()), this, SLOT(slot1Response()));

    /* 30001 + 2512 = 32513: Modbus function 04, first TIT register 2512. */
    if (!w.rd_reg(32513U, 12U))
    {
        slot1Finish(false, QString::fromUtf8("Неверный адрес Modbus"));
        return;
    }
    slot1TimeoutTimer->start(1500);
}

void ex::slot1Response()
{
    slot1TimeoutTimer->stop();
    disconnect(&w, SIGNAL(s_rd()), this, SLOT(slot1Response()));

    const int functionOffset = 1 + w.ElamFlag;
    const int dataOffset = w.ElamFlag ? functionOffset + 3 : functionOffset + 2;
    const int expectedBytes = 24;
    const int receivedBytes = w.ElamFlag
            ? (((int) w.buf_in[functionOffset + 1] << 8) |
               w.buf_in[functionOffset + 2])
            : w.buf_in[functionOffset + 1];
    if (!w.CRC_ok || w.max_in < (unsigned int) (dataOffset + expectedBytes + 2) ||
        w.buf_in[functionOffset] != 4 || receivedBytes < expectedBytes)
    {
        slot1Finish(false, QString::fromUtf8("Ошибка ответа или CRC"));
        return;
    }

    quint16 words[12];
    for (int i = 0; i < 12; ++i)
        words[i] = (quint16) w.get_word1(dataOffset + 1 + i * 2);

    union
    {
        quint32 bits;
        float value;
    } converted;
    converted.bits = ((quint32) words[0] << 16) | words[1];

    slot1Value->setText(QString::number(converted.value, 'g', 8));
    slot1Raw->setText(QString("TIT[2512]=0x%1, TIT[2513]=0x%2")
                      .arg(words[0], 4, 16, QChar('0'))
                      .arg(words[1], 4, 16, QChar('0')).toUpper());

    const qint16 commStatus = (qint16) words[10];
    if (commStatus == 0)
    {
        slot1Comm->setText(QString::fromUtf8("OK (0)"));
        slot1Comm->setStyleSheet("color: #167d2d;");
    }
    else
    {
        slot1Comm->setText(QString::fromUtf8("Ошибка %1").arg(commStatus));
        slot1Comm->setStyleSheet("color: #b02020;");
    }
    slot1Success->setText(QString::number(words[11]));
    slot1Finish(true, QString::fromUtf8("Данные получены с COM6"));
}

void ex::slot1Timeout()
{
    disconnect(&w, SIGNAL(s_rd()), this, SLOT(slot1Response()));
    slot1Finish(false, QString::fromUtf8("Нет ответа от COM6"));
}

void ex::slot1Finish(bool success, const QString &message)
{
    slot1TimeoutTimer->stop();
    slot1Busy = false;
    slot1Status->setText(message + QString::fromUtf8(" — ") +
                         QDateTime::currentDateTime().toString("hh:mm:ss"));
    slot1Status->setStyleSheet(success ? "color: #167d2d;" : "color: #b02020;");
}

void ex::profilerTabChanged(int index)
{
    if (ui->tabWidget->widget(index) == profilerTab)
    {
        if (profilerAuto->isChecked())
            profilerPollTimer->start();
        profilerStartCycle();
    }
    else
    {
        profilerPollTimer->stop();
    }
}

void ex::profilerAutoChanged(bool enabled)
{
    if (enabled && ui->tabWidget->currentWidget() == profilerTab)
    {
        profilerPollTimer->start();
        profilerStartCycle();
    }
    else
    {
        profilerPollTimer->stop();
    }
}

void ex::profilerRefresh()
{
    profilerStartCycle();
}

void ex::profilerEnable()
{
    if (profilerBusy || objectProgramPhase != ObjectProgramIdle)
        return;
    profilerPollTimer->stop();
    profilerEnableActive = true;
    profilerEnableButton->setEnabled(false);
    profilerStatus->setText(QString::fromUtf8(
        "Включение профилировщика через %1...")
        .arg(w.com_S ? "UDP" : "COM6"));
    disconnect(&w, SIGNAL(s_rd()), 0, 0);
    connect(&w, SIGNAL(s_rd()), this, SLOT(profilerEnableResponse()));
    if (!w.wr_reg(40001U + 8490U, 1U))
    {
        disconnect(&w, SIGNAL(s_rd()), this, SLOT(profilerEnableResponse()));
        profilerEnableActive = false;
        profilerEnableButton->setEnabled(true);
        profilerStatus->setText(QString::fromUtf8(
            "Не удалось отправить команду включения"));
        return;
    }
    profilerTimeoutTimer->start(1500);
}

void ex::profilerEnableResponse()
{
    profilerTimeoutTimer->stop();
    disconnect(&w, SIGNAL(s_rd()), this, SLOT(profilerEnableResponse()));
    profilerEnableActive = false;
    profilerEnableButton->setEnabled(true);
    if (!w.CRC_ok)
    {
        profilerStatus->setText(QString::fromUtf8(
            "Ошибка ответа включения профилировщика"));
        return;
    }
    profilerStatus->setText(QString::fromUtf8("Профилировщик включён"));
    if (profilerAuto->isChecked())
        profilerPollTimer->start();
    QTimer::singleShot(300, this, SLOT(profilerRefresh()));
}

void ex::profilerPoll()
{
    if (ui->tabWidget->currentWidget() == profilerTab)
        profilerStartCycle();
}

void ex::profilerStartCycle()
{
    if (profilerBusy || objectProgramPhase != ObjectProgramIdle)
        return;
    if (!w.com_S && !w.port->isOpen())
    {
        profilerFinish(false, QString::fromUtf8("COM6 не открыт"));
        return;
    }

    profilerBusy = true;
    profilerReadingHeader = true;
    profilerHeader.clear();
    profilerThreadData.clear();
    profilerThreadCount = 0;
    profilerNextRegister = 0;
    profilerStatus->setText(QString::fromUtf8("Чтение заголовка через %1…")
                            .arg(w.com_S ? "UDP" : "COM6"));
    profilerRead(8000U, 10U);
}

void ex::profilerRead(unsigned int address, unsigned int count)
{
    profilerRequestAddress = address;
    profilerRequestCount = count;
    disconnect(&w, SIGNAL(s_rd()), 0, 0);
    connect(&w, SIGNAL(s_rd()), this, SLOT(profilerResponse()));
    if (!w.rd_reg(40001U + address, count))
    {
        profilerFinish(false, QString::fromUtf8("Неверный адрес Modbus"));
        return;
    }
    profilerTimeoutTimer->start(1500);
}

void ex::profilerResponse()
{
    profilerTimeoutTimer->stop();
    disconnect(&w, SIGNAL(s_rd()), this, SLOT(profilerResponse()));

    const int functionOffset = 1 + w.ElamFlag;
    const int dataOffset = w.ElamFlag ? functionOffset + 3 : functionOffset + 2;
    const int expectedBytes = (int) profilerRequestCount * 2;
    const int receivedBytes = w.ElamFlag
            ? (((int) w.buf_in[functionOffset + 1] << 8) |
               w.buf_in[functionOffset + 2])
            : w.buf_in[functionOffset + 1];
    if (!w.CRC_ok || w.max_in < (unsigned int) (dataOffset + expectedBytes + 2) ||
        w.buf_in[functionOffset] != 3 || receivedBytes < expectedBytes)
    {
        profilerFinish(false, QString::fromUtf8("Ошибка ответа или CRC"));
        return;
    }

    QVector<quint16> words;
    words.reserve((int) profilerRequestCount);
    for (unsigned int i = 0; i < profilerRequestCount; ++i)
        words.append((quint16) w.get_word1(dataOffset + 1 + (int) i * 2));

    if (profilerReadingHeader)
    {
        profilerHeader = words;
        if (profilerHeader.size() < 10 || profilerHeader.at(0) != 0x5052U ||
            profilerHeader.at(2) == 0U)
        {
            profilerFinish(false, QString::fromUtf8("Профилировщик в контроллере не активен"));
            return;
        }
        profilerThreadCount = qMin((int) profilerHeader.at(3), 24);
        profilerThreadData.resize(profilerThreadCount * 20);
        profilerReadingHeader = false;
        if (profilerThreadCount == 0)
        {
            profilerRender();
            profilerFinish(true, QString::fromUtf8("Потоки пока не зарегистрированы"));
            return;
        }
        profilerStatus->setText(QString::fromUtf8("Чтение потоков: 0/%1")
                                .arg(profilerThreadCount));
        profilerRead(8010U, qMin(591, profilerThreadCount * 20));
        return;
    }

    const int destination = (int) profilerRequestAddress - 8010;
    for (int i = 0; i < words.size() && destination + i < profilerThreadData.size(); ++i)
        profilerThreadData[destination + i] = words.at(i);
    profilerNextRegister = destination + words.size();

    if (profilerNextRegister < profilerThreadData.size())
    {
        const int completedThreads = profilerNextRegister / 20;
        profilerStatus->setText(QString::fromUtf8("Чтение потоков: %1/%2")
                                .arg(completedThreads).arg(profilerThreadCount));
        const int left = profilerThreadData.size() - profilerNextRegister;
        profilerRead(8010U + profilerNextRegister, qMin(591, left));
        return;
    }

    profilerRender();
    profilerFinish(true, QString::fromUtf8("Данные получены через %1")
                   .arg(w.com_S ? "UDP" : "COM6"));
}

void ex::profilerTimeout()
{
    if (profilerEnableActive)
    {
        disconnect(&w, SIGNAL(s_rd()), this, SLOT(profilerEnableResponse()));
        profilerEnableActive = false;
        profilerEnableButton->setEnabled(true);
        profilerStatus->setText(QString::fromUtf8(
            "Нет ответа на команду включения через %1")
            .arg(w.com_S ? "UDP" : "COM6"));
        return;
    }
    disconnect(&w, SIGNAL(s_rd()), this, SLOT(profilerResponse()));
    profilerFinish(false, QString::fromUtf8("Нет ответа через %1")
                   .arg(w.com_S ? "UDP" : "COM6"));
}

void ex::profilerFinish(bool success, const QString &message)
{
    profilerTimeoutTimer->stop();
    profilerBusy = false;
    profilerStatus->setText(message + QString::fromUtf8(" — ") +
                            QDateTime::currentDateTime().toString("hh:mm:ss"));
    profilerStatus->setStyleSheet(success ? "color: #167d2d;" : "color: #b02020;");
}

quint32 ex::profilerU32(const QVector<quint16> &data, int offset)
{
    if (offset < 0 || offset + 1 >= data.size())
        return 0;
    return ((quint32) data.at(offset) << 16) | data.at(offset + 1);
}

quint64 ex::profilerU64(const QVector<quint16> &data, int offset)
{
    if (offset < 0 || offset + 3 >= data.size())
        return 0;
    quint64 value = 0;
    for (int i = 0; i < 4; ++i)
        value = (value << 16) | data.at(offset + i);
    return value;
}

void ex::profilerRender()
{
    if (profilerHeader.size() >= 7)
    {
        const double load = profilerHeader.at(4) / 100.0;
        const quint32 windowUs = profilerU32(profilerHeader, 5);
        profilerLoad->setText(QString::fromUtf8("CPU без idle: %1 %").arg(load, 0, 'f', 2));
        profilerWindow->setText(QString::fromUtf8("Окно: %1 мкс").arg(windowUs));
    }

    profilerTable->setRowCount(profilerThreadCount);
    for (int row = 0; row < profilerThreadCount; ++row)
    {
        const int base = row * 20;
        QByteArray nameBytes;
        for (int i = 0; i < 8; ++i)
        {
            const quint16 word = profilerThreadData.value(base + i);
            const char first = (char) (word >> 8);
            const char second = (char) (word & 0xff);
            if (first)
                nameBytes.append(first);
            if (second)
                nameBytes.append(second);
        }
        const quint16 cpuX100 = profilerThreadData.value(base + 8);
        const quint32 windowUs = profilerU32(profilerThreadData, base + 9);
        const quint64 totalUs = profilerU64(profilerThreadData, base + 11);
        const quint32 maxRunUs = profilerU32(profilerThreadData, base + 15);
        const quint32 switches = profilerU32(profilerThreadData, base + 17);
        const quint16 priorityState = profilerThreadData.value(base + 19);

        QStringList values;
        values << QString::fromLatin1(nameBytes.constData(), nameBytes.size())
               << QString::number(cpuX100 / 100.0, 'f', 2)
               << QString::number((windowUs + 500U) / 1000U)
               << QString::number(windowUs)
               << QString::number(totalUs)
               << QString::number(maxRunUs)
               << QString::number(switches)
               << QString("%1 / 0x%2")
                     .arg(priorityState >> 8)
                     .arg(priorityState & 0xff, 2, 16, QChar('0'));
        for (int column = 0; column < values.size(); ++column)
        {
            QTableWidgetItem *item = profilerTable->item(row, column);
            if (!item)
            {
                item = new QTableWidgetItem;
                profilerTable->setItem(row, column, item);
            }
            item->setText(values.at(column));
            if (column > 0)
                item->setTextAlignment(Qt::AlignRight | Qt::AlignVCenter);
        }
    }
}

void ex::on_comboBox_6_currentIndexChanged(int index)
{
    sht(index);
    w.ss=w2.kns[index];
}
void ex::sh(bool par)
{
    QString ss,s1;
    unsigned int k;
    if (par)    ss="->";    else    ss="<-"  ;
    if(w.com_S)if (par){
        for(k=0;k<22;k++) ss +=s1.sprintf("%02x ",w.buf_out[k]);
        ss +='{';
         }else {
        for(k = 0;k < 10;k++)
            ss +=s1.sprintf("%02x ",w.buf_in[k]);
                    ss +='{';
                    };
    if (par){
        if(w.com_S)
            for(k=0;k<w.max_out;k++) ss +=s1.sprintf("0x%02x ",w.buf_out[k+22]);
            else
              for(k=0;k<w.max_out;k++) ss +=s1.sprintf("0x%02x ",w.buf_out[k]);
        }
    else  { for(k=0;k<w.max_in;k++)   ss +=s1.sprintf("0x%02x ",w.buf_in[k]);
    ss +=s1.sprintf(" max_in %d ",w.max_in);}
    if(w.com_S)  ss +='}';
    if(!par)if( w.CRC_ok) ss+=" ok"; else  ss+="ERROR";
    ui->scr->append(ss);
    if (w.enn)
        for(k = 0;k <w.max_in;k++)
             w.buf_in[k]=w.buf_in[k+9];
}
void ex::sht(int k)
{ QString ssr;
    //QStringList ss;
     u16 j;
    ss = w2.kns[k];
   ui->lineEdit_115->setText(ss[1]);
   ui->lineEdit_116->setText(ss[6]);
   ui->lineEdit_117->setText(ss[7]);
   ui->lineEdit_118->setText(ss[8]);
   j=ss[2].toInt();
   switch(j){
   case 3:ssr="1200";break;
   case 4:ssr="2400";break;
   case 5:ssr="4800";break;
   case 6:ssr="9600";break;
   case 7:ssr="14400";break;
   case 8:ssr="19200";break;
   case 9:ssr="38400";break;
   case 10:ssr="56000";break;
   case 11:ssr="57600";break;
   }
   ui->lineEdit_119->setText(ssr);
   j=ss[4].toInt();
   switch(j){
   case 0:ssr="1";break;
   case 1:ssr="1,5";break;
   case 2:ssr="2";break;
      }
   ui->lineEdit_120->setText(ssr);
   j=ss[5].toInt();
   switch(j){
   case 0:ssr="no";break;
   case 1:ssr="even";break;
   case 2:ssr="odd";break;
      }
    ui->lineEdit_121->setText(ssr);
    j=ss[9].toInt();
    switch(j){
    case 1:ssr="rs485_1";break;
    case 2:ssr="rs485_2";break;
    case 3:ssr="rs232_1";break;
    case 4:ssr="rs232_2";break;
       }
     ui->lineEdit_122->setText(ssr);
     ui->lineEdit_123->setText(ss[10]);
}

void ex::on_pushButton_166_clicked()
{

    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_csq()));
  w.rd_reg(31934,2);
  sh(1);
}
void ex::sh_csq()
{
    sh(0);
    float dat;
    dat = w.get_real(7+w.ElamFlag); //
    ui->lineEdit_137->setText(QString::number(dat));
}

void ex::on_pushButton_167_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_s_sim()));
    w.rd_reg(31922,3);
    sh(1);
}
void ex::sh_s_sim()
{
    sh(0);
    WORD hh;
    hh = w.get_word1(5+w.ElamFlag); //
    ui->lineEdit_149->setText(QString::number(hh));
    hh = w.get_word1(7+w.ElamFlag); //
    ui->lineEdit_138->setText(QString::number(hh));
   }

void ex::on_pushButton_171_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ip()));
    w.rd_reg(44889,7);//
     sh(1);
}
void ex::sh_ip()
{
    sh(0);
    WORD hh;
    QString ff;
    hh = w.get_word1(5+w.ElamFlag);    ff += QString::number(hh)+ ".";
    hh = w.get_word1(7+w.ElamFlag);    ff += QString::number(hh) + ".";
    hh = w.get_word1(9+w.ElamFlag);    ff += QString::number(hh) + ".";
    hh = w.get_word1(11+w.ElamFlag);   ff += QString::number(hh);
    ui->lineEdit_151->setText(ff);
    hh = w.get_word1(13+w.ElamFlag);
    ui->lineEdit_152->setText(QString::number(hh,10));
    hh = w.get_word1(15+w.ElamFlag);
    ui->lineEdit_150->setText(QString::number(hh,10));
    hh = w.get_word1(17+w.ElamFlag);
    ui->lineEdit_153->setText(QString::number(hh,10));
}

void ex::on_pushButton_174_clicked()
{
    QString ff;
   ff += QString::number(217)+ "." +QString::number(198)+ "." +QString::number(10)+ "." +QString::number(164);
   ui->lineEdit_151->setText(ff);
   ui->lineEdit_152->setText(QString::number(5000,10));
}

void ex::on_pushButton_172_clicked()
{
    unsigned int tt[16];
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    QStringList ws=ui->lineEdit_151->text().split(".");
    for(int k=0;k<ws.count();k++)
        tt[k]=ws.at(k).toInt();
    tt[4]=ui->lineEdit_152->text().toInt();
    tt[5]=ui->lineEdit_150->text().toInt();
    tt[6]=ui->lineEdit_153->text().toInt();
    w.wr_mas(44889,7,&tt[0]);
      sh(1);
}

void ex::on_comboBox_currentIndexChanged(int index)
{
    switch (index){
    case 0: w.Baud1 = BAUD4800; break;
    case 1: w.Baud1 = BAUD9600; break;
    case 2: w.Baud1 = BAUD14400; break;
    case 3: w.Baud1 = BAUD19200; break;
    case 4: w.Baud1 = BAUD38400; break;
    case 5: w.Baud1 = BAUD56000; break;
    case 6: w.Baud1 = BAUD57600; break;
    case 7: w.Baud1 = BAUD115200; break;
               }
w.ini_c();
}

void ex::on_comboBox_3_currentIndexChanged(const QString &arg1)
{
    const int index = ui->comboBox_3->currentIndex();
    if (index < 0 || !ui->comboBox_3->isEnabled())
        return;
    const QString deviceName = ui->comboBox_3->itemData(index).toString();
    w.port_name = deviceName.isEmpty() ? arg1 : deviceName;
}

void ex::on_pushButton_clicked()
{
    if( w.ini_c()==true)
     ui->label_4->setText("ok");
        else  ui->label_4->setText("ERROR");
}

void ex::on_pushButton_27_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ts()));
    w.rd_reg(30001,1);//
    sh(1);
}
void ex::sh_ts()
{
   BYTE k;
   QString s1;
   sh(0);
   WORD dat;
   qDebug("max_inS: %d", 5);
   dat = w.get_word1(5+w.ElamFlag); //
   //ui->label_28->setText(QString::number(dat,16));
   ui->textEdit->clear();
   for(k=0;k<16;k++){
       //s1 = "N:"+ QString::number(k)+" ";
       s1 = QString("%1:").arg(k+1,2);
   if((dat & (1<<k)))ui->textEdit->append(s1+"1");
       else ui->textEdit->append(s1+".");
                  }

}

void ex::on_pushButton_41_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_tii3()));
    w.rd_reg(30015,16);//
    sh(1);
}
void ex::sh_tii3()
{

    sh(0);
    WORD hh,k;
    ui->textEdit_4->clear();
    for(k=0;k<16;k++){
     hh = w.get_word1(5+w.ElamFlag+k*2); //
     ui->textEdit_4->append(QString("%1:%2").arg(k+1,2).arg(hh,6));
    }
}

void ex::on_checkBox_28_clicked()
{
    if(ui->checkBox_28->isChecked())w.com_S=1;else w.com_S=0;
}

void ex::on_pushButton_40_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_tit12()));
    w.rd_reg(30002,12);//
    sh(1);
}
void ex::sh_tit12()
{

    sh(0);
    WORD hh,k;
    ui->textEdit_3->clear();
    for(k=0;k<12;k++){
     hh = w.get_word1(5+w.ElamFlag+k*2); //
     ui->textEdit_3->append(QString("%1:%2").arg(k+1,2).arg(hh,-6));
    }
}

void ex::on_pushButton_43_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_tit_float()));
    w.rd_reg(30033,24);//
    sh(1);
}
void ex::sh_tit_float()
{

    sh(0);
    WORD k;
    float dat;
    ui->textEdit_5->clear();
    for(k=0;k<12;k++){
        dat = w.get_real(7+k*4+w.ElamFlag);
        //if(dat==0)test_0[2]++;
        ui->textEdit_5->append(QString("%1:%2").arg(k+1,2).arg(dat,4,'F',3));
    }
//ui->lineEdit_23->setText(QString("%1").arg(test_0[2]));
}

void ex::on_pushButton_2_clicked()
{

}

void ex::on_lineEdit_118_textChanged(const QString &arg1)
{

}

void ex::on_lineEdit_118_editingFinished()
{
     w.ss[8]=ui->lineEdit_118->text();
}

//void ex::on_pushButton_4_clicked()
//{

//}

//void ex::on_lineEdit_14_cursorPositionChanged(int arg1, int arg2)
//{

//}

//void ex::on_pushButton_13_clicked()
//{

//}

void ex::on_get_ver_rd_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ver()));
    w.rd_reg(31963,1);//
    sh(1);
}
void ex::sh_ver()
{

    sh(0);
    unsigned int dat;
    dat = w.get_word1(5+w.ElamFlag);
    ui->get_ver_line->setText(QString::number(dat));

}
//void ex::set_func( void (*f)(void))
//{
//    //disconnect(&w,SIGNAL(s_rd()),0,0);
//    //connect(&w,SIGNAL(s_rd()),this,SLOT((*f)()));
//}


void ex::on_ini_par_ty_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    w.wr_ty(847,1);
    sh(1);
}

void ex::on_time_rd_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_d()));
    w.rd_reg(41425,6);//
    sh(1);
   }
void ex::sh_d()
{
    sh(0);
    WORD ss;
     ss = w.get_word1(5+w.ElamFlag); ui->time_year->setText(QString::number(ss));
     ss = w.get_word1(7+w.ElamFlag); ui->time_m->setText(QString::number(ss));
     ss = w.get_word1(9+w.ElamFlag); ui->time_day->setText(QString::number(ss));
     ss = w.get_word1(11+w.ElamFlag); ui->time_hour->setText(QString::number(ss));
     ss = w.get_word1(13+w.ElamFlag); ui->time_min->setText(QString::number(ss));
     ss = w.get_word1(15+w.ElamFlag); ui->time_sek->setText(QString::number(ss));

}

void ex::on_time_wr_clicked()
{
    unsigned  int tt[6];
    QString s;
    WORD rr;
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
s = ui->time_year->text(); rr = s.toInt();tt[0]=rr; //max
s = ui->time_m->text(); rr = s.toInt(); tt[1]=rr;  // min
s = ui->time_day->text(); rr = s.toInt();tt[2]=rr; //max
s = ui->time_hour->text(); rr = s.toInt(); tt[3]=rr;  // min
s = ui->time_min->text(); rr = s.toInt(); tt[4]=rr;  // min
s = ui->time_sek->text(); rr = s.toInt(); tt[5]=rr;  // min
w.wr_mas(41425,6,&tt[0]);//
    sh(1);
}
void ex::sh_ty()
{
    sh(0);

}
void ex::on_time_set_2_clicked()
{
    QString ss;
    QDate dd = QDate::currentDate();
    QTime tt = QTime::currentTime();
    ui->time_year->setText(dd.toString("yyyy"));
    ui->time_m->setText(dd.toString("MM"));
    ui->time_day->setText(dd.toString("dd"));
    ui->time_hour->setText(tt.toString("HH"));
    ui->time_min->setText(tt.toString("mm"));
    ui->time_sek->setText(tt.toString("s"));
}

void ex::on_set_t1_rd_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_tii_time()));
    w.rd_reg(40242,5);//
     sh(1);
}
void ex::sh_tii_time() //
   {
    sh(0);
    u16 r2;
   r2=w.get_word1(5+w.ElamFlag);   if(r2<16)   ui->set_t1_box->setCurrentIndex(r2);
    }

void ex::on_set_t1_wr_clicked()
{
    unsigned int tt[7];
    QString s;
    u16 r;
   disconnect(&w,SIGNAL(s_rd()),0,0);
   connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
   r= ui->set_t1_box->currentIndex();  tt[0]=r;
   tt[1]=0;
   w.wr_mas(40242,1,&tt[0]);//
   sh(1);
}

void ex::on_pushButton_51_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_tii_time2()));
    w.rd_reg(40251,1);//
    sh(1);
}
void ex::sh_tii_time2() //
   {
    sh(0);
    u16 r2;
   r2=w.get_word1(5+w.ElamFlag);
   if(r2<14)
   ui->set_t2_box->setCurrentIndex(r2);

   }


void ex::on_tit_rd_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_n_f()));
    w.rd_reg(40017,48);//
    sh(1);
    }
void ex::sh_n_f()
    {
    float dat;
        sh(0);
    for(int k=0;k < 12;k++)
      {
      dat = w.get_real(7 + k*8 + w.ElamFlag);
      ui->table_kp->item(k,0)->setText(QString::number(dat));
      dat = w.get_real(11 + k*8 + w.ElamFlag);
      ui->table_kp->item(k,1)->setText(QString::number(dat));
         }
    }

void ex::on_tit_rd_2_clicked()
{
    float rr;
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    unsigned int tt[48];
    for(int k=0;k<12;k++){
        rr= ui->table_kp->item(k,0)->text().toFloat(); w.setMbFloat1(&tt[k*4],rr);
        rr= ui->table_kp->item(k,1)->text().toFloat(); w.setMbFloat1(&tt[k*4+2],rr);
     }
        w.wr_mas(40017,48,&tt[0]);//
    sh(1);
}

void ex::on_tit_setd_clicked()
{
    u16 k;
    for(k=0;k<12;k++){
    ui->table_kp->item(k,0)->setText("6");
    ui->table_kp->item(k,1)->setText("0");
             }
}

void ex::on_pushButton_278_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_mbF()));
    w.rd_reg(40252,2);//
    sh(1);
}
void ex::sh_mbF()
{
    sh(0);
    WORD r2;
     r2 = w.get_word1(5+w.ElamFlag); //
    if(r2 & 0x0001) ui->mb1->setChecked(1);else ui->mb1->setChecked(0);
    if(r2 & 0x0002) ui->mb1_2->setChecked(1);else ui->mb1_2->setChecked(0);
    if(r2 & 0x0004) ui->mb1_3->setChecked(1);else ui->mb1_3->setChecked(0);
    if(r2 & 0x0008) ui->mb1_4->setChecked(1);else ui->mb1_4->setChecked(0);
    if(r2 & 0x0010) ui->mb1_5->setChecked(1);else ui->mb1_5->setChecked(0);
    if(r2 & 0x0020) ui->mb1_6->setChecked(1);else ui->mb1_6->setChecked(0);
    if(r2 & 0x0040) ui->mb1_7->setChecked(1);else ui->mb1_7->setChecked(0);
    if(r2 & 0x0080) ui->mb1_8->setChecked(1);else ui->mb1_8->setChecked(0);
    if(r2 & 0x0100) ui->mb1_9->setChecked(1);else ui->mb1_9->setChecked(0);
    if(r2 & 0x0200) ui->mb1_10->setChecked(1);else ui->mb1_10->setChecked(0);
    if(r2 & 0x0400) ui->mb1_11->setChecked(1);else ui->mb1_11->setChecked(0);
    if(r2 & 0x0800) ui->mb1_12->setChecked(1);else ui->mb1_12->setChecked(0);
    if(r2 & 0x1000) ui->mb1_13->setChecked(1);else ui->mb1_13->setChecked(0);
    if(r2 & 0x2000) ui->mb1_14->setChecked(1);else ui->mb1_14->setChecked(0);
    if(r2 & 0x4000) ui->mb1_15->setChecked(1);else ui->mb1_15->setChecked(0);
    if(r2 & 0x8000) ui->mb1_16->setChecked(1);else ui->mb1_16->setChecked(0);

    r2 = w.get_word1(7+w.ElamFlag); //
   if(r2 & 0x0001) ui->mb2->setChecked(1);else ui->mb2->setChecked(0);
   if(r2 & 0x0002) ui->mb2_2->setChecked(1);else ui->mb2_2->setChecked(0);
   if(r2 & 0x0004) ui->mb2_3->setChecked(1);else ui->mb2_3->setChecked(0);
   if(r2 & 0x0008) ui->mb2_4->setChecked(1);else ui->mb2_4->setChecked(0);
   if(r2 & 0x0010) ui->mb2_5->setChecked(1);else ui->mb2_5->setChecked(0);
   if(r2 & 0x0020) ui->mb2_6->setChecked(1);else ui->mb2_6->setChecked(0);
   if(r2 & 0x0040) ui->mb2_7->setChecked(1);else ui->mb2_7->setChecked(0);
   if(r2 & 0x0080) ui->mb2_8->setChecked(1);else ui->mb2_8->setChecked(0);
   if(r2 & 0x0100) ui->mb2_9->setChecked(1);else ui->mb2_9->setChecked(0);
   if(r2 & 0x0200) ui->mb2_10->setChecked(1);else ui->mb2_10->setChecked(0);
   if(r2 & 0x0400) ui->mb2_11->setChecked(1);else ui->mb2_11->setChecked(0);
   if(r2 & 0x0800) ui->mb2_12->setChecked(1);else ui->mb2_12->setChecked(0);
   if(r2 & 0x1000) ui->mb2_13->setChecked(1);else ui->mb2_13->setChecked(0);
   if(r2 & 0x2000) ui->mb2_14->setChecked(1);else ui->mb2_14->setChecked(0);
   if(r2 & 0x4000) ui->mb2_15->setChecked(1);else ui->mb2_15->setChecked(0);
   if(r2 & 0x8000) ui->mb2_16->setChecked(1);else ui->mb2_16->setChecked(0);


}

void ex::on_pushButton_279_clicked()
{
    unsigned int tt[4];
    QString s;
    u16 r,r1;
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
   tt[0]=0;tt[1]=0;
   if(ui->mb1->isChecked()) tt[0] |=0x0001;
   if(ui->mb1_2->isChecked()) tt[0] |=0x0002;
   if(ui->mb1_3->isChecked()) tt[0] |=0x0004;
   if(ui->mb1_4->isChecked()) tt[0] |=0x0008;
   if(ui->mb1_5->isChecked()) tt[0] |=0x0010;
   if(ui->mb1_6->isChecked()) tt[0] |=0x0020;
   if(ui->mb1_7->isChecked()) tt[0] |=0x0040;
   if(ui->mb1_8->isChecked()) tt[0] |=0x0080;
   if(ui->mb1_9->isChecked()) tt[0] |=0x0100;
   if(ui->mb1_10->isChecked()) tt[0] |=0x0200;
   if(ui->mb1_11->isChecked()) tt[0] |=0x0400;
   if(ui->mb1_12->isChecked()) tt[0] |=0x0800;
   if(ui->mb1_13->isChecked()) tt[0] |=0x1000;
   if(ui->mb1_14->isChecked()) tt[0] |=0x2000;
   if(ui->mb1_15->isChecked()) tt[0] |=0x4000;
   if(ui->mb1_16->isChecked()) tt[0] |=0x8000;

   if(ui->mb2->isChecked()) tt[1] |=0x0001;
   if(ui->mb2_2->isChecked()) tt[1] |=0x0002;
   if(ui->mb2_3->isChecked()) tt[1] |=0x0004;
   if(ui->mb2_4->isChecked()) tt[1] |=0x0008;
   if(ui->mb2_5->isChecked()) tt[1] |=0x0010;
   if(ui->mb2_6->isChecked()) tt[1] |=0x0020;
   if(ui->mb2_7->isChecked()) tt[1] |=0x0040;
   if(ui->mb2_8->isChecked()) tt[1] |=0x0080;
   if(ui->mb2_9->isChecked()) tt[1] |=0x0100;
   if(ui->mb2_10->isChecked()) tt[1] |=0x0200;
   if(ui->mb2_11->isChecked()) tt[1] |=0x0400;
   if(ui->mb2_12->isChecked()) tt[1] |=0x0800;
   if(ui->mb2_13->isChecked()) tt[1] |=0x1000;
   if(ui->mb2_14->isChecked()) tt[1] |=0x2000;
   if(ui->mb2_15->isChecked()) tt[1] |=0x4000;
   if(ui->mb2_16->isChecked()) tt[1] |=0x8000;

w.wr_mas(40252,2,&tt[0]);//
sh(1);
}


void ex::on_B_sos_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_sos2()));
    w.rd_reg(31841,32);//
    sh(1);
}
void ex::sh_sos2()
{
    sh(0);
    WORD hh[14],k;
    QString ss;
    ui->textEdit_17->clear();
    for(k=0;k<4;k++){
        hh[0] = w.get_word1(5+k*16+w.ElamFlag); //
        hh[1] = w.get_word1(7+k*16+w.ElamFlag); //
        hh[2] = w.get_word1(9+k*16+w.ElamFlag); //
        hh[3] = w.get_word1(11+k*16+w.ElamFlag); //
        hh[4] = w.get_word1(13+k*16+w.ElamFlag); //
        hh[5] = w.get_word1(15+k*16+w.ElamFlag); //
        hh[6] = w.get_word1(17+k*16+w.ElamFlag); //
        hh[7] = w.get_word1(19+k*16+w.ElamFlag); //
     ss=QString("%1: %2 %3 %4 %5 %5 %6 %7 %8 %9 ").arg(k*8+1,-6).arg(hh[0],-6,16).arg(hh[1],-6,16).arg(hh[2],-6,16)
             .arg(hh[3],-6,16).arg(hh[4],-6,16).arg(hh[5],-6,16).arg(hh[6],-6,16).arg(hh[7],-6,16);
     ui->textEdit_17->append(ss);
                    }
}


void ex::on_B_zam_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_zamb()));
    w.rd_reg(33101,64);//
    sh(1);
}
void ex::sh_zamb()
{
    sh(0);
    float hh[14],k;
    QString ss;
    ui->textEdit_17->clear();
    for(k=0;k<4;k++){
        hh[0] = w.get_real(7+k*32+w.ElamFlag); //
        hh[1] = w.get_real(11+k*32+w.ElamFlag); //
        hh[2] = w.get_real(15+k*32+w.ElamFlag); //
        hh[3] = w.get_real(19+k*32+w.ElamFlag); //
        hh[4] = w.get_real(23+k*32+w.ElamFlag); //
        hh[5] = w.get_real(27+k*32+w.ElamFlag); //
        hh[6] = w.get_real(31+k*32+w.ElamFlag); //
        hh[7] = w.get_real(35+k*32+w.ElamFlag); //
        ss="";
        qDebug("hh %f  %f %f %f", hh[0],hh[1],hh[2],hh[3]);
        for(int l=0;l<8;l++)
        ss+=QString("%1:%2  ").arg(k*8+l+1,-6).arg(hh[l],-6);
        ui->textEdit_17->append(ss);
    //ui->textEdit_17->append(QString("%1: n-%2 ok-%3 er-%4").arg(k*2+2,-6).arg(hh[4],-6).arg(hh[5],-6).arg(hh[7],-6));
                }
}

void ex::on_pushButton_283_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_er2()));
    w.rd_reg(31501,128);//
    sh(1);
}
void ex::sh_er2()
{
    sh(0);
    WORD hh[14],k;
    QString ss;
    ui->textEdit_17->clear();
    for(k=0;k<16;k++){
        hh[0] = w.get_word1(5+k*16+w.ElamFlag); //
        hh[1] = w.get_word1(7+k*16+w.ElamFlag); //
        hh[2] = w.get_word1(9+k*16+w.ElamFlag); //
        hh[3] = w.get_word1(11+k*16+w.ElamFlag); //
        hh[4] = w.get_word1(13+k*16+w.ElamFlag); //
        hh[5] = w.get_word1(15+k*16+w.ElamFlag); //
        hh[6] = w.get_word1(17+k*16+w.ElamFlag); //
        hh[7] = w.get_word1(19+k*16+w.ElamFlag); //
        ss=QString("%1: n-%2 ok-%3 er-%4 ").arg(k*2+1,-6).arg(hh[0],-6).arg(hh[1],-6).arg(hh[3],-6);
        ss+=QString("%1: n-%2 ok-%3 er-%4 ").arg(k*2+2,-6).arg(hh[4],-6).arg(hh[5],-6).arg(hh[7],-6);
     ui->textEdit_17->append(ss);
    //ui->textEdit_17->append(QString("%1: n-%2 ok-%3 er-%4").arg(k*2+2,-6).arg(hh[4],-6).arg(hh[5],-6).arg(hh[7],-6));
                }
}


void ex::on_pushButton_139_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_indzz()));
    w.rd_reg(30401,1);//
    sh(1);
}
void ex::sh_indzz(void) //
{
 sh(0);
 unsigned int dat;
 dat = w.get_word1(5+w.ElamFlag);
 ind_st_zz=dat;
 ui->lineEdit_76->setText(QString::number(ind_st_zz));
 }

void ex::on_pushButton_140_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_arx_zz()));
   w.rd_reg1(0x2000+(ind_st_zz*74),74);
     if(ind_st_zz)ind_st_zz--;else ind_st_zz =546;  //
     ui->lineEdit_76->setText(QString::number(ind_st_zz)); //
    sh(1);
}
void ex::sh_arx_zz(void) //
{
 QString ss;
 float z;
 QDateTime d;
 DWORD datt;
 u16 k,k1;
 sh(0);
 datt = w.get_u32(7+w.ElamFlag);
 d.setTime_t(datt);
 ss = d.toString("dd/MM/yyyy HH:mm:ss ");// + QString::number(datt,16);
 ui->textEdit_17->append(ss);
 ss="";
 z =  w.get_real1(11+w.ElamFlag);
 ss+= QString::number(1)+"{"+ QString::number(z)+"} ";
 z =  w.get_real1(15+w.ElamFlag);
 ss+= QString::number(2)+"{"+ QString::number(z)+"} ";
 z =  w.get_real1(19+w.ElamFlag);
 ss+= QString::number(3)+"{"+ QString::number(z)+"} ";

 z =  w.get_real1(23+w.ElamFlag);
 ss+= QString::number(4)+"{"+ QString::number(z)+"} ";
  ui->textEdit_17->append(ss);


 for(k1=0;k1<4;k1++){
     ss="";
     for(k=0;k<8;k++){
             z =  w.get_real1(11+k*4+k1*32+w.ElamFlag);
             ss+= QString::number(k+1+k1*8)+"{"+ QString::number(z)+"} ";
                      }
    ui->textEdit_17->append(ss);

                      }

}

void ex::on_pushButton_145_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_indzz1()));
    w.rd_reg(30402,1);//
    sh(1);
}
void ex::sh_indzz1(void) //
{
 sh(0);
 unsigned int dat;
 dat = w.get_word1(5+w.ElamFlag);
 ui->lineEdit_77->setText(QString::number(dat));
 ind_st_zz1=dat;
}

void ex::on_pushButton_146_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_arx_zz1()));
   w.rd_reg1(0xD000+(ind_st_zz1*4),4);
     if(ind_st_zz1)ind_st_zz1--;else ind_st_zz1 =1530;  //
     ui->lineEdit_77->setText(QString::number(ind_st_zz1)); //
    sh(1);
}
void ex::sh_arx_zz1(void) //
{
    QString ss;
    sh(0);

    WORD k,z,z1;
    DWORD datt;
    QDateTime d;
    datt = w.get_u32(7+w.ElamFlag);
    d.setTime_t(datt);
    ss = d.toString("dd/MM/yyyy HH:mm:ss ");// + QString::number(datt,16);
     z1 = w.get_word1(9+w.ElamFlag);
     if(z1<33)
        ss+= " rtu:"+ QString::number(z1); //obv
     if(z1==60)
        ss+= " ts:";
     if(z1==70)
        ss+= "reset:";

    z = w.get_word1(11+w.ElamFlag);
   if(z1<33) ss+= " состояние:"+ QString::number(z,16); //obv
   if(z1==60) ss+= " состояние:"+ QString::number(z,16); //obv
   if(z1==70) ss+= " количество:"+ QString::number(z); //obv
     ui->textEdit_17->append(ss);


}


void ex::on_pushButton_144_clicked()
{
    ui->textEdit_17->clear();
}

void ex::on_pushButton_280_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);   connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    w.wr_ty(847,1);
    sh(1);
}

void ex::on_pushButton_141_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);   connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    w.wr_ty(10000,1);
    sh(1);
}

void ex::on_pushButton_142_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);   connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    w.wr_ty(10001,1);
    sh(1);
}

void ex::on_pushButton_149_clicked()
{
    ui->scr->clear();
}

void ex::on_pushButton_147_clicked()
{

}

void ex::on_pushButton_148_clicked()
{

}

void ex::on_p_ind1_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ind_t1()));
    w.rd_reg(30401,1);//
    sh(1);
}
void ex::sh_ind_t1(void) //
{
 sh(0);
 unsigned int dat;
 dat = w.get_word1(5+w.ElamFlag);
 ind_t1=dat;
 ui->l_t1->setText(QString::number(ind_t1));
 }

void ex::on_pushButton_153_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_arx_t1()));
   w.rd_reg1(10000+(ind_t1*18),18);
  // w.rd_reg1(10018,18);
     if(ind_t1)ind_t1--;else ind_t1 =150;  //
     ui->l_t1->setText(QString::number(ind_t1)); //
    sh(1);
}
void ex::sh_arx_t1(void) //
{
    QString ss,ss1,ss2,ss3,ss4,ss5;
    sh(0);
     float ff1,ff2;
    WORD tts[10],k,z;
    DWORD datt;
    QDateTime d;
    datt = w.get_u32(7+w.ElamFlag);
    d.setTime_t(datt);
    ss = d.toString("dd/MM/yyyy HH:mm:ss ");// + QString::number(datt,16);
    for(k=0;k<16;k++){
    z = w.get_word1(9+k*2+w.ElamFlag);
    ss+= QString::number(k+1)+"{"+ QString::number(z)+"} "; //obv
   }
      ui->textEdit_17->append(ss);

}

void ex::on_l_t1_textChanged(const QString &arg1)
{
    ind_t1=arg1.toInt();
}

void ex::on_ini_par_ty_2_clicked()
{
//dd.show();
}

void ex::on_pushButton_52_clicked()
{

}


void ex::on_p_bl_on_clicked()
{
    unsigned int tt[7];
    QString s;
    u16 r;
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    tt[0]=ui->sp_slot->value();
    w.wr_mas2(12048,1,&tt[0]);//
    sh(1);
}


void ex::on_p_bl_onn_clicked()
{
    unsigned int tt[7];
    QString s;
    u16 r;
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    r= ui->set_t1_box->currentIndex();  tt[0]=r;
    tt[0]=0xa503;
    w.wr_mas2(12050,1,&tt[0]);//
    sh(1);
}


void ex::on_p_bl_off_clicked()
{
    unsigned int tt[7];
    QString s;
    u16 r;
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    r= ui->set_t1_box->currentIndex();  tt[0]=r;
    tt[0]=0xa504;
    w.wr_mas2(12050,1,&tt[0]);//
    sh(1);
}

/*
 * Включает опрос демонстрационного модуля UART7.
 * wr_reg() формирует Modbus function 06; адрес 42501 преобразуется в 2500.
 */
void ex::on_p_poll_on_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    w.wr_reg(42501,1);
    sh(1);
}

/*
 * Выключает опрос демонстрационного модуля UART7 через TIT[2500]=0.
 */
void ex::on_p_poll_off_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    w.wr_reg(42501,0);
    sh(1);
}


void ex::on_p_sos_slot_clicked()
{
        disconnect(&w,SIGNAL(s_rd()),0,0);
        connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ssos()));
        w.rd_reg(32451,4);//
        sh(1);
       }
void ex::sh_ssos(void) //
{
        sh(0);
        u16 r2;
        ui->t_ssos->clear();
        r2=w.get_word1(5+w.ElamFlag);
        ui->t_ssos->append("состояние:" +QString::number(r2));
        r2=w.get_word1(6+w.ElamFlag);
        ui->t_ssos->append("код:" +QString::number(r2));
        r2=w.get_word1(8+w.ElamFlag);
        ui->t_ssos->append("n_slot:" +QString::number(r2));

}


void ex::on_p_sos_slot_2_clicked()
{
        disconnect(&w,SIGNAL(s_rd()),0,0);
        connect(&w,SIGNAL(s_rd()),this,SLOT(sh_s2500()));
        w.rd_reg(32501,5);//
        sh(1);
       }
void ex::sh_s2500(void) //
{
        sh(0);
        u16 r2;
        ui->t_ssos->clear();
        r2=w.get_word1(5+w.ElamFlag);
        ui->t_ssos->append("вкл:" +QString::number(r2));
        r2=w.get_word1(6+w.ElamFlag);
        ui->t_ssos->append("период:" +QString::number(r2));
        r2=w.get_word1(8+w.ElamFlag);
        ui->t_ssos->append("масштаб:" +QString::number(r2));
        r2=w.get_word1(9+w.ElamFlag);
        ui->t_ssos->append("масштаб:" +QString::number(r2));
        r2=w.get_word1(10+w.ElamFlag);
        ui->t_ssos->append("тип float:" +QString::number(r2));
        r2=w.get_word1(11+w.ElamFlag);
        ui->t_ssos->append("arx = 1:" +QString::number(r2));

}

void ex::objectProgramLog(const QString &message)
{
    objectLog->append(message);
    statusBar()->showMessage(message);
}

void ex::setupTagConfigPage()
{
    tagConfigPhase = TagConfigIdle;
    tagConfigCommand = 0U;
    tagConfigGeneration = 0U;
    tagConfigActivePort = 0U;
    tagConfigActiveDevice = 1U;
    tagConfigFrameIndex = 0;
    tagConfigPollCount = 0;
    tagConfigExpectedLength = 0;

    tagConfigTab = new QWidget(ui->tabWidget);
    ui->tabWidget->addTab(tagConfigTab, QString::fromUtf8("Теги FlashDB"));
    QVBoxLayout *layout = new QVBoxLayout(tagConfigTab);

    QHBoxLayout *controls = new QHBoxLayout;
    controls->addWidget(new QLabel(QString::fromUtf8("RS-485 порт:"), tagConfigTab));
    tagConfigPortSpin = new QSpinBox(tagConfigTab);
    tagConfigPortSpin->setRange(1, TagConfigProtocol::PortCount);
    controls->addWidget(tagConfigPortSpin);
    controls->addWidget(new QLabel(QString::fromUtf8("Устройство:"), tagConfigTab));
    tagConfigDeviceSpin = new QSpinBox(tagConfigTab);
    tagConfigDeviceSpin->setRange(1, TagConfigProtocol::DeviceCount);
    tagConfigDeviceSpin->setValue(1);
    controls->addWidget(tagConfigDeviceSpin);
    tagConfigReadButton = new QPushButton(QString::fromUtf8("Прочитать"), tagConfigTab);
    tagConfigSaveButton = new QPushButton(QString::fromUtf8("Сохранить и применить"), tagConfigTab);
    tagConfigActivateButton = new QPushButton(QString::fromUtf8("Загрузить в RAM"), tagConfigTab);
    controls->addWidget(tagConfigReadButton);
    controls->addWidget(tagConfigSaveButton);
    controls->addWidget(tagConfigActivateButton);
    controls->addStretch(1);
    tagConfigGenerationLabel = new QLabel(QString::fromUtf8("Поколение: 0"), tagConfigTab);
    controls->addWidget(tagConfigGenerationLabel);
    layout->addLayout(controls);

    tagConfigTable = new QTableWidget(
        TagConfigProtocol::TagsPerDevice, 13, tagConfigTab);
    tagConfigTable->setHorizontalHeaderLabels(QStringList()
        << QString::fromUtf8("Вкл")
        << QString::fromUtf8("Тег")
        << QString::fromUtf8("Тип")
        << QString::fromUtf8("Имя")
        << QString::fromUtf8("Ед.")
        << QString::fromUtf8("Источник")
        << QString::fromUtf8("Адрес")
        << QString::fromUtf8("Период, мс")
        << QString::fromUtf8("Масштаб")
        << QString::fromUtf8("Смещение")
        << QString::fromUtf8("Запись")
        << QString::fromUtf8("Архив")
        << QString::fromUtf8("Порядок"));
    tagConfigTable->verticalHeader()->setVisible(false);
    tagConfigTable->setAlternatingRowColors(true);
    tagConfigTable->horizontalHeader()->setResizeMode(QHeaderView::ResizeToContents);
    tagConfigTable->horizontalHeader()->setResizeMode(3, QHeaderView::Stretch);

    for (int row = 0; row < TagConfigProtocol::TagsPerDevice; ++row)
    {
        QCheckBox *enabled = new QCheckBox(tagConfigTable);
        tagConfigTable->setCellWidget(row, 0, enabled);
        QTableWidgetItem *sensor =
            new QTableWidgetItem(QString::number(row + 1));
        sensor->setFlags(sensor->flags() & ~Qt::ItemIsEditable);
        sensor->setTextAlignment(Qt::AlignCenter);
        tagConfigTable->setItem(row, 1, sensor);

        QComboBox *type = new QComboBox(tagConfigTable);
        type->addItems(QStringList() << "float32" << "bool" << "uint16"
                                    << "int16" << "uint32" << "int32");
        tagConfigTable->setCellWidget(row, 2, type);
        tagConfigTable->setItem(row, 3, new QTableWidgetItem);
        tagConfigTable->setItem(row, 4, new QTableWidgetItem);

        QComboBox *source = new QComboBox(tagConfigTable);
        source->addItems(QStringList() << QString::fromUtf8("Нет")
                         << "Modbus" << "TIT");
        tagConfigTable->setCellWidget(row, 5, source);
        QSpinBox *address = new QSpinBox(tagConfigTable);
        address->setRange(0, 65535);
        tagConfigTable->setCellWidget(row, 6, address);
        QSpinBox *poll = new QSpinBox(tagConfigTable);
        poll->setRange(0, 60000);
        poll->setValue(1000);
        tagConfigTable->setCellWidget(row, 7, poll);
        QDoubleSpinBox *scale = new QDoubleSpinBox(tagConfigTable);
        scale->setDecimals(6);
        scale->setRange(-1000000.0, 1000000.0);
        scale->setValue(1.0);
        tagConfigTable->setCellWidget(row, 8, scale);
        QDoubleSpinBox *offset = new QDoubleSpinBox(tagConfigTable);
        offset->setDecimals(6);
        offset->setRange(-1000000.0, 1000000.0);
        tagConfigTable->setCellWidget(row, 9, offset);
        tagConfigTable->setCellWidget(row, 10, new QCheckBox(tagConfigTable));
        tagConfigTable->setCellWidget(row, 11, new QCheckBox(tagConfigTable));
        QComboBox *wordOrder = new QComboBox(tagConfigTable);
        wordOrder->addItems(QStringList() << "ABCD" << "CDAB"
                                         << "BADC" << "DCBA");
        tagConfigTable->setCellWidget(row, 12, wordOrder);
    }
    layout->addWidget(tagConfigTable, 1);

    tagConfigStatus = new QLabel(
        QString::fromUtf8("Порты 1…5 = UART1…UART5 с аппаратным OE/DE; UART7/8 не опрашиваются"),
        tagConfigTab);
    layout->addWidget(tagConfigStatus);

    TagConfigProtocol::Row initial;
    initial.enabled = true;
    initial.sensor = 1;
    initial.type = 0;
    initial.flags = 0x04;
    initial.sourceKind = 2;
    initial.wordOrder = 0;
    initial.sourceAddress = 2512;
    initial.pollMs = 1000;
    initial.scale = 1.0f;
    initial.offset = 0.0f;
    initial.name = "uart2.device1.float27";
    initial.unit = "";
    QVector<TagConfigProtocol::Row> defaults;
    defaults.append(initial);
    tagConfigRender(defaults);

    tagConfigTimeoutTimer = new QTimer(this);
    tagConfigTimeoutTimer->setSingleShot(true);
    connect(tagConfigTimeoutTimer, SIGNAL(timeout()),
            this, SLOT(tagConfigTimeout()));
    connect(tagConfigReadButton, SIGNAL(clicked()),
            this, SLOT(tagConfigRead()));
    connect(tagConfigSaveButton, SIGNAL(clicked()),
            this, SLOT(tagConfigSave()));
    connect(tagConfigActivateButton, SIGNAL(clicked()),
            this, SLOT(tagConfigActivate()));
    connect(tagConfigPortSpin, SIGNAL(valueChanged(int)),
            this, SLOT(tagConfigSelectionChanged()));
    connect(tagConfigDeviceSpin, SIGNAL(valueChanged(int)),
            this, SLOT(tagConfigSelectionChanged()));
}

QVector<TagConfigProtocol::Row> ex::tagConfigRows() const
{
    QVector<TagConfigProtocol::Row> rows;
    for (int row = 0; row < TagConfigProtocol::TagsPerDevice; ++row)
    {
        TagConfigProtocol::Row item;
        QCheckBox *enabled = qobject_cast<QCheckBox *>(
            tagConfigTable->cellWidget(row, 0));
        QComboBox *type = qobject_cast<QComboBox *>(
            tagConfigTable->cellWidget(row, 2));
        QComboBox *source = qobject_cast<QComboBox *>(
            tagConfigTable->cellWidget(row, 5));
        QSpinBox *address = qobject_cast<QSpinBox *>(
            tagConfigTable->cellWidget(row, 6));
        QSpinBox *poll = qobject_cast<QSpinBox *>(
            tagConfigTable->cellWidget(row, 7));
        QDoubleSpinBox *scale = qobject_cast<QDoubleSpinBox *>(
            tagConfigTable->cellWidget(row, 8));
        QDoubleSpinBox *offset = qobject_cast<QDoubleSpinBox *>(
            tagConfigTable->cellWidget(row, 9));
        QCheckBox *writable = qobject_cast<QCheckBox *>(
            tagConfigTable->cellWidget(row, 10));
        QCheckBox *archive = qobject_cast<QCheckBox *>(
            tagConfigTable->cellWidget(row, 11));
        QComboBox *wordOrder = qobject_cast<QComboBox *>(
            tagConfigTable->cellWidget(row, 12));
        item.enabled = enabled && enabled->isChecked();
        item.sensor = quint8(row + 1);
        item.type = quint8(type ? type->currentIndex() : 0);
        item.flags = (writable && writable->isChecked() ? 0x02 : 0) |
                     (archive && archive->isChecked() ? 0x04 : 0);
        item.sourceKind = quint8(source ? source->currentIndex() : 0);
        item.wordOrder = quint8(wordOrder ? wordOrder->currentIndex() : 0);
        item.sourceAddress = quint16(address ? address->value() : 0);
        item.pollMs = quint16(poll ? poll->value() : 0);
        item.scale = float(scale ? scale->value() : 1.0);
        item.offset = float(offset ? offset->value() : 0.0);
        item.name = tagConfigTable->item(row, 3) ?
                    tagConfigTable->item(row, 3)->text() : QString();
        item.unit = tagConfigTable->item(row, 4) ?
                    tagConfigTable->item(row, 4)->text() : QString();
        rows.append(item);
    }
    return rows;
}

void ex::tagConfigRender(const QVector<TagConfigProtocol::Row> &rows)
{
    for (int row = 0; row < TagConfigProtocol::TagsPerDevice; ++row)
    {
        TagConfigProtocol::Row item;
        item.enabled = false;
        item.sensor = quint8(row + 1);
        item.type = 0;
        item.flags = 0;
        item.sourceKind = 0;
        item.wordOrder = 0;
        item.sourceAddress = 0;
        item.pollMs = 1000;
        item.scale = 1.0f;
        item.offset = 0.0f;
        for (int index = 0; index < rows.size(); ++index)
            if (rows.at(index).sensor == row + 1)
                item = rows.at(index);
        qobject_cast<QCheckBox *>(tagConfigTable->cellWidget(row, 0))
            ->setChecked(item.enabled);
        qobject_cast<QComboBox *>(tagConfigTable->cellWidget(row, 2))
            ->setCurrentIndex(item.type);
        tagConfigTable->item(row, 3)->setText(item.name);
        tagConfigTable->item(row, 4)->setText(item.unit);
        qobject_cast<QComboBox *>(tagConfigTable->cellWidget(row, 5))
            ->setCurrentIndex(item.sourceKind);
        qobject_cast<QSpinBox *>(tagConfigTable->cellWidget(row, 6))
            ->setValue(item.sourceAddress);
        qobject_cast<QSpinBox *>(tagConfigTable->cellWidget(row, 7))
            ->setValue(item.pollMs);
        qobject_cast<QDoubleSpinBox *>(tagConfigTable->cellWidget(row, 8))
            ->setValue(item.scale);
        qobject_cast<QDoubleSpinBox *>(tagConfigTable->cellWidget(row, 9))
            ->setValue(item.offset);
        qobject_cast<QCheckBox *>(tagConfigTable->cellWidget(row, 10))
            ->setChecked((item.flags & 0x02U) != 0U);
        qobject_cast<QCheckBox *>(tagConfigTable->cellWidget(row, 11))
            ->setChecked((item.flags & 0x04U) != 0U);
        qobject_cast<QComboBox *>(tagConfigTable->cellWidget(row, 12))
            ->setCurrentIndex(item.wordOrder);
    }
}

void ex::tagConfigRead()
{
    tagConfigStart(TagConfigProtocol::CommandRead);
}

void ex::tagConfigSelectionChanged()
{
    if (tagConfigPhase != TagConfigIdle)
        return;
    tagConfigGeneration = 0U;
    tagConfigGenerationLabel->setText(
        QString::fromUtf8("Поколение: —"));
    tagConfigRender(QVector<TagConfigProtocol::Row>());
    tagConfigStatus->setStyleSheet(QString());
    tagConfigStatus->setText(QString::fromUtf8(
        "Выбран порт %1, устройство %2 — нажмите «Прочитать»")
        .arg(tagConfigPortSpin->value())
        .arg(tagConfigDeviceSpin->value()));
}

void ex::tagConfigSave()
{
    tagConfigStart(TagConfigProtocol::CommandSaveApply);
}

void ex::tagConfigActivate()
{
    tagConfigStart(TagConfigProtocol::CommandActivate);
}

void ex::tagConfigStart(quint16 command)
{
    QString error;
    if (tagConfigPhase != TagConfigIdle ||
        objectProgramPhase != ObjectProgramIdle ||
        profilerBusy || slot1Busy || floatConfigBusy)
        return;
    if (!w.com_S && !w.port->isOpen())
    {
        tagConfigFinish(false, QString::fromUtf8("COM-порт не открыт"));
        return;
    }

    tagConfigCommand = command;
    tagConfigActivePort = quint8(tagConfigPortSpin->value() - 1);
    tagConfigActiveDevice = quint8(tagConfigDeviceSpin->value());
    tagConfigFrameIndex = 0;
    tagConfigPollCount = 0;
    tagConfigBlob.clear();
    if (command == TagConfigProtocol::CommandSaveApply)
    {
        quint32 generation = tagConfigGeneration + 1U;
        if (!generation) generation = 1U;
        if (!TagConfigProtocol::buildBlob(
                tagConfigActivePort,
                tagConfigActiveDevice,
                generation, tagConfigRows(),
                &tagConfigBlob, &error))
        {
            tagConfigFinish(false, error);
            return;
        }
        tagConfigFrames = TagConfigProtocol::makeDataWriteFrames(
            tagConfigBlob, w.mb_rtu);
        if (tagConfigFrames.isEmpty())
        {
            tagConfigFinish(false, QString::fromUtf8("Не удалось сформировать данные"));
            return;
        }
        tagConfigPortSpin->setEnabled(false);
        tagConfigDeviceSpin->setEnabled(false);
        tagConfigPhase = TagConfigUpload;
        tagConfigStatus->setText(QString::fromUtf8("Передача конфигурации…"));
        disconnect(&w, SIGNAL(s_rd()), 0, 0);
        connect(&w, SIGNAL(s_rd()), this, SLOT(tagConfigResponse()));
        tagConfigSend(tagConfigFrames.first());
        return;
    }

    tagConfigPhase = TagConfigControl;
    tagConfigPortSpin->setEnabled(false);
    tagConfigDeviceSpin->setEnabled(false);
    tagConfigStatus->setText(command == TagConfigProtocol::CommandRead ?
        QString::fromUtf8("Чтение конфигурации FlashDB…") :
        QString::fromUtf8("Загрузка конфигурации в RAM…"));
    disconnect(&w, SIGNAL(s_rd()), 0, 0);
    connect(&w, SIGNAL(s_rd()), this, SLOT(tagConfigResponse()));
    tagConfigSend(TagConfigProtocol::makeControlFrame(
        0U, tagConfigActivePort, tagConfigActiveDevice,
        command, w.mb_rtu));
}

void ex::tagConfigSend(const QByteArray &frame)
{
    if (frame.isEmpty() || frame.size() > int(sizeof(w.buf_out)))
    {
        tagConfigFinish(false, QString::fromUtf8("Не удалось сформировать Modbus-кадр"));
        return;
    }
    tagConfigLastRequest = frame;
    w.max_out = frame.size();
    memcpy(w.buf_out, frame.constData(), frame.size());
    w.CRC_ok = false;
    w.send_current_frame();
    sh(1);
    tagConfigTimeoutTimer->start(3000);
}

void ex::tagConfigPoll()
{
    if (tagConfigPhase == TagConfigStatus)
        tagConfigSend(TagConfigProtocol::makeStatusReadFrame(w.mb_rtu));
}

void ex::tagConfigResponse()
{
    tagConfigTimeoutTimer->stop();
    sh(0);
    QByteArray response(reinterpret_cast<const char *>(w.buf_in), w.max_in);
    QString error;

    if (!w.CRC_ok)
    {
        tagConfigFinish(false, QString::fromUtf8("CRC ответа или ответ не получен"));
        return;
    }
    if (tagConfigPhase == TagConfigUpload)
    {
        if (!TagConfigProtocol::isWriteAcknowledge(
                tagConfigLastRequest, response, &error))
        {
            tagConfigFinish(false, error);
            return;
        }
        ++tagConfigFrameIndex;
        if (tagConfigFrameIndex < tagConfigFrames.size())
            tagConfigSend(tagConfigFrames.at(tagConfigFrameIndex));
        else
        {
            tagConfigPhase = TagConfigControl;
            tagConfigSend(TagConfigProtocol::makeControlFrame(
                quint16(tagConfigBlob.size()),
                tagConfigActivePort,
                tagConfigActiveDevice,
                tagConfigCommand, w.mb_rtu));
        }
        return;
    }
    if (tagConfigPhase == TagConfigControl)
    {
        if (!TagConfigProtocol::isWriteAcknowledge(
                tagConfigLastRequest, response, &error))
        {
            tagConfigFinish(false, error);
            return;
        }
        tagConfigPhase = TagConfigStatus;
        QTimer::singleShot(100, this, SLOT(tagConfigPoll()));
        return;
    }
    if (tagConfigPhase == TagConfigStatus)
    {
        TagConfigProtocol::Status status;
        if (!TagConfigProtocol::parseStatus(response, &status, &error))
        {
            tagConfigFinish(false, error);
            return;
        }
        if (status.status == TagConfigProtocol::StatusBusy)
        {
            if (++tagConfigPollCount > 100)
                tagConfigFinish(false, QString::fromUtf8("Тайм-аут FlashDB"));
            else
                QTimer::singleShot(100, this, SLOT(tagConfigPoll()));
            return;
        }
        if (status.status == TagConfigProtocol::StatusError)
        {
            tagConfigFinish(false, QString::fromUtf8("FlashDB: ошибка %1")
                            .arg(status.result));
            return;
        }
        if (tagConfigCommand == TagConfigProtocol::CommandRead &&
            status.status == TagConfigProtocol::StatusReady)
        {
            if (status.length < TagConfigProtocol::HeaderSize ||
                status.length > TagConfigProtocol::MaxBlobSize)
            {
                tagConfigFinish(false, QString::fromUtf8("Неверная длина блока"));
                return;
            }
            tagConfigExpectedLength = status.length;
            tagConfigGeneration = status.generation;
            tagConfigFrames = TagConfigProtocol::makeDataReadFrames(
                status.length, w.mb_rtu);
            tagConfigFrameIndex = 0;
            tagConfigBlob.clear();
            tagConfigPhase = TagConfigDownload;
            tagConfigSend(tagConfigFrames.first());
            return;
        }
        if ((tagConfigCommand == TagConfigProtocol::CommandSaveApply ||
             tagConfigCommand == TagConfigProtocol::CommandActivate) &&
            status.status == TagConfigProtocol::StatusComplete)
        {
            if (tagConfigCommand == TagConfigProtocol::CommandSaveApply)
                tagConfigGeneration = status.generation;
            tagConfigFinish(true, tagConfigCommand ==
                TagConfigProtocol::CommandSaveApply ?
                QString::fromUtf8("Сохранено во FlashDB и загружено в RAM") :
                QString::fromUtf8("Конфигурация загружена в RAM"));
            return;
        }
        tagConfigFinish(false, QString::fromUtf8("Неожиданный статус %1")
                        .arg(status.status));
        return;
    }
    if (tagConfigPhase == TagConfigDownload)
    {
        if (!TagConfigProtocol::parseReadBytes(
                response, &tagConfigBlob, &error))
        {
            tagConfigFinish(false, error);
            return;
        }
        ++tagConfigFrameIndex;
        if (tagConfigFrameIndex < tagConfigFrames.size())
        {
            tagConfigSend(tagConfigFrames.at(tagConfigFrameIndex));
            return;
        }
        tagConfigBlob.truncate(tagConfigExpectedLength);
        QVector<TagConfigProtocol::Row> rows;
        quint32 generation;
        if (!TagConfigProtocol::parseBlob(
                tagConfigBlob, tagConfigActivePort,
                tagConfigActiveDevice,
                &rows, &generation, &error))
        {
            tagConfigFinish(false, error);
            return;
        }
        tagConfigGeneration = generation;
        tagConfigRender(rows);
        tagConfigFinish(true, QString::fromUtf8("Конфигурация прочитана: %1 тегов")
                        .arg(rows.size()));
    }
}

void ex::tagConfigTimeout()
{
    tagConfigFinish(false, QString::fromUtf8("Нет ответа от контроллера"));
}

void ex::tagConfigFinish(bool success, const QString &message)
{
    tagConfigTimeoutTimer->stop();
    disconnect(&w, SIGNAL(s_rd()), this, SLOT(tagConfigResponse()));
    tagConfigPhase = TagConfigIdle;
    tagConfigPortSpin->setEnabled(true);
    tagConfigDeviceSpin->setEnabled(true);
    tagConfigGenerationLabel->setText(
        QString::fromUtf8("Поколение: %1").arg(tagConfigGeneration));
    tagConfigStatus->setText(message + QString::fromUtf8(" — ") +
                             QDateTime::currentDateTime().toString("hh:mm:ss"));
    tagConfigStatus->setStyleSheet(success ?
        "color: #167d2d;" : "color: #b02020;");
    if (luaTagsRefreshActive)
    {
        luaTagsTable->setRowCount(0);
        if (success)
        {
            const QVector<TagConfigProtocol::Row> rows = tagConfigRows();
            for (int index = 0; index < rows.size(); ++index)
            {
                const TagConfigProtocol::Row &tag = rows.at(index);
                if (!tag.enabled)
                    continue;
                const int row = luaTagsTable->rowCount();
                luaTagsTable->insertRow(row);
                luaTagsTable->setItem(row, 0, new QTableWidgetItem(
                    QString::number(tagConfigActivePort + 1)));
                luaTagsTable->setItem(row, 1, new QTableWidgetItem(
                    QString::number(tagConfigActiveDevice)));
                luaTagsTable->setItem(row, 2, new QTableWidgetItem(
                    QString::number(tag.sensor)));
                luaTagsTable->setItem(row, 3, new QTableWidgetItem(tag.name));
                luaTagsTable->setItem(row, 4, new QTableWidgetItem(
                    QString::number(tag.type)));
                luaTagsTable->setItem(row, 5, new QTableWidgetItem("-"));
                luaTagsTable->setItem(row, 6, new QTableWidgetItem(
                    QString("0x%1").arg(tag.flags, 2, 16, QChar('0'))));
            }
            const unsigned int base = 14000U +
                ((unsigned int)tagConfigActivePort * 30U * 30U +
                 ((unsigned int)tagConfigActiveDevice - 1U) * 30U) * 4U;
            disconnect(&w, SIGNAL(s_rd()), 0, 0);
            connect(&w, SIGNAL(s_rd()), this, SLOT(luaTagsValuesResponse()));
            luaShowStatus(QString::fromUtf8(
                "UDP: чтение текущих значений tag_registry..."), true);
            if (!w.rd_reg(40001U + base, 120U))
            {
                luaTagsValuesTimeout();
                return;
            }
            luaTagsValuesTimer->start(2000);
        }
        else
        {
            luaTagsRefreshActive = false;
            luaSetBusy(false);
            luaShowStatus(message, false);
        }
    }
}

void ex::luaTagsValuesResponse()
{
    luaTagsValuesTimer->stop();
    const int functionOffset = 1 + w.ElamFlag;
    const int dataOffset = w.ElamFlag ? functionOffset + 3 : functionOffset + 2;
    const int receivedBytes = w.ElamFlag ?
        (((int)w.buf_in[functionOffset + 1] << 8) |
         w.buf_in[functionOffset + 2]) : w.buf_in[functionOffset + 1];
    if (!w.CRC_ok || w.buf_in[functionOffset] != 3 || receivedBytes != 8)
    {
        luaTagsValuesTimeout();
        return;
    }
    quint16 words[4];
    for (int index = 0; index < 4; ++index)
        words[index] = (quint16)w.get_word1(dataOffset + 1 + index * 2);
    const int row = luaTagKeyIndex;
    const quint16 typeFlags = words[1];
    const quint8 type = quint8(typeFlags >> 8);
    const quint8 flags = quint8(typeFlags);
    const quint32 bits = (quint32(words[2]) << 16) | words[3];
    QString value;
    if (type == 0)
    {
        float number;
        memcpy(&number, &bits, sizeof(number));
        value = QString::number(number, 'g', 8);
    }
    else if (type == 1 || type == 2) value = QString::number(bits);
    else if (type == 3) value = QString::number((qint16)bits);
    else if (type == 4) value = QString::number(bits);
    else if (type == 5) value = QString::number((qint32)bits);
    else value = "-";
    luaTagsTable->item(row, 4)->setText(QString::number(type));
    luaTagsTable->item(row, 5)->setText(value);
    luaTagsTable->item(row, 6)->setText(
        QString("0x%1").arg(flags, 2, 16, QChar('0')));
    ++luaTagKeyIndex;
    QTimer::singleShot(80, this, SLOT(luaTagsContinueRead()));
    return;
#if 0
    for (int sensor = 1; sensor <= 30; ++sensor)
    {
        const int offset = (sensor - 1) * 4;
        const quint8 flags = quint8(words.at(offset + 1));
        if (flags == 0U)
            continue;
        bool present = false;
        for (int row = 0; row < luaTagsTable->rowCount(); ++row)
            if (luaTagsTable->item(row, 2) &&
                luaTagsTable->item(row, 2)->text().toInt() == sensor)
            {
                present = true;
                break;
            }
        if (!present)
        {
            const int row = luaTagsTable->rowCount();
            luaTagsTable->insertRow(row);
            luaTagsTable->setItem(row, 0, new QTableWidgetItem(
                QString::number(tagConfigActivePort + 1)));
            luaTagsTable->setItem(row, 1, new QTableWidgetItem(
                QString::number(tagConfigActiveDevice)));
            luaTagsTable->setItem(row, 2, new QTableWidgetItem(
                QString::number(sensor)));
            luaTagsTable->setItem(row, 3, new QTableWidgetItem(
                QString("tag_%1_%2_%3")
                    .arg(tagConfigActivePort + 1)
                    .arg(tagConfigActiveDevice).arg(sensor)));
            luaTagsTable->setItem(row, 4, new QTableWidgetItem("-"));
            luaTagsTable->setItem(row, 5, new QTableWidgetItem("-"));
            luaTagsTable->setItem(row, 6, new QTableWidgetItem("-"));
        }
    }
    for (int row = 0; row < luaTagsTable->rowCount(); ++row)
    {
        const int sensor = luaTagsTable->item(row, 2)->text().toInt();
        if (sensor < 1 || sensor > 30)
            continue;
        const int offset = (sensor - 1) * 4;
        const quint16 typeFlags = words.at(offset + 1);
        const quint8 type = quint8(typeFlags >> 8);
        const quint8 flags = quint8(typeFlags);
        const quint32 bits = (quint32(words.at(offset + 2)) << 16) |
                             words.at(offset + 3);
        QString value;
        if (type == 0)
        {
            float number;
            memcpy(&number, &bits, sizeof(number));
            value = QString::number(number, 'g', 8);
        }
        else if (type == 1 || type == 2)
            value = QString::number(bits);
        else if (type == 3)
            value = QString::number((qint16)bits);
        else if (type == 4)
            value = QString::number(bits);
        else if (type == 5)
            value = QString::number((qint32)bits);
        else
            value = "-";
        luaTagsTable->item(row, 4)->setText(QString::number(type));
        luaTagsTable->item(row, 5)->setText(value);
        luaTagsTable->item(row, 6)->setText(
            QString("0x%1").arg(flags, 2, 16, QChar('0')));
    }
    luaTagsRefreshActive = false;
    luaSetBusy(false);
    luaShowStatus(QString::fromUtf8("UDP: значения tag_registry обновлены (%1)")
                  .arg(luaTagsTable->rowCount()), true);
#endif
}

void ex::luaTagsContinueRead()
{
    if (luaTagsRefreshActive)
        luaStartNextTagValueRead();
}

void ex::luaTagsValuesTimeout()
{
    luaTagsValuesTimer->stop();
    luaTagsAutoTimer->stop();
    disconnect(&w, SIGNAL(s_rd()), this, SLOT(luaTagsValuesResponse()));
    luaTagsRefreshActive = false;
    luaSetBusy(false);
    luaShowStatus(QString::fromUtf8(
        "Нет ответа при чтении значений tag_registry по UDP"), false);
}

void ex::objectProgramTypeChanged(int)
{
    const int type = objectTypeCombo->itemData(
        objectTypeCombo->currentIndex()).toInt();
    const bool web = type == ObjectProgramming::ObjectWebFile;
    const bool executable =
        type == ObjectProgramming::ObjectXipModule ||
        type == ObjectProgramming::ObjectLuaVm;
    objectContentTypeLabel->setVisible(web);
    objectContentTypeCombo->setVisible(web);
    objectApiVersionLabel->setVisible(executable);
    objectApiVersionSpin->setVisible(executable);
    objectLinkAddressLabel->setVisible(executable);
    objectLinkAddressEdit->setVisible(executable);
    objectEntryOffsetLabel->setVisible(executable);
    objectEntryOffsetSpin->setVisible(executable);
    objectAutostartCheck->setVisible(executable);
    objectCompressedCheck->setEnabled(!executable);
    if (executable)
        objectCompressedCheck->setChecked(false);

    if (!objectFileEdit->text().isEmpty())
    {
        const QString defaultName = ObjectProgramming::defaultNameForFile(
            objectFileEdit->text(), quint16(type));
        objectNameEdit->setText(defaultName);
        if (web)
        {
            quint32 objectId =
                ObjectProgramming::crc32(defaultName.toUtf8()) & 0x7FFFFFFFU;
            objectIdSpin->setValue(objectId ? objectId : 1U);
        }
        if (web)
            objectContentTypeCombo->setCurrentIndex(
                ObjectProgramming::contentTypeForFile(
                    objectFileEdit->text()) - 1);
    }
}

void ex::objectProgramOpen()
{
    const QDir modulesDir(QCoreApplication::applicationDirPath() +
                          "/../../../4/modules");
    if (modulesDir.exists())
        QDir::setCurrent(modulesDir.absolutePath());
    const QString fileName = QFileDialog::getOpenFileName(
        this, tr("Выберите payload для OBJ1"), QString(),
        tr("Все файлы (*.*)"));
    if (fileName.isEmpty())
        return;
    QFile file(fileName);
    if (!file.open(QIODevice::ReadOnly))
    {
        objectProgramFail(tr("Не удалось открыть %1: %2")
                          .arg(fileName).arg(file.errorString()));
        return;
    }
    const QByteArray payload = file.readAll();
    if (payload.isEmpty() ||
        quint32(payload.size()) >
        quint32(ObjectProgramming::ObjectAreaBlockCount) *
        ObjectProgramming::BlockSize - ObjectProgramming::HeaderSize)
    {
        objectProgramFail(tr("Payload пуст или не помещается в OBJ1-область"));
        return;
    }
    objectProgramPayload = payload;
    objectProgramImage.clear();
    objectFileEdit->setText(fileName);
    const QString payloadName = QFileInfo(fileName).fileName().toLower();
    if (payloadName == "profiler.bin")
    {
        const int typeIndex = objectTypeCombo->findData(
            ObjectProgramming::ObjectXipModule);
        if (typeIndex >= 0)
            objectTypeCombo->setCurrentIndex(typeIndex);
        objectIdSpin->setValue(2);
        objectLinkAddressEdit->setText("0x90005080");
        objectEntryOffsetSpin->setValue(0);
        objectApiVersionSpin->setValue(5);
    }
    else if (payloadName == "lua_vm.bin")
    {
        const int typeIndex = objectTypeCombo->findData(
            ObjectProgramming::ObjectLuaVm);
        if (typeIndex >= 0)
            objectTypeCombo->setCurrentIndex(typeIndex);
        objectIdSpin->setValue(5);
        objectLinkAddressEdit->setText("0x9002E080");
        objectEntryOffsetSpin->setValue(0);
        objectApiVersionSpin->setValue(5);
    }
    const quint16 type = quint16(objectTypeCombo->itemData(
        objectTypeCombo->currentIndex()).toInt());
    const QString defaultName =
        ObjectProgramming::defaultNameForFile(fileName, type);
    objectNameEdit->setText(defaultName);
    if (type == ObjectProgramming::ObjectWebFile)
    {
        quint32 objectId =
            ObjectProgramming::crc32(defaultName.toUtf8()) & 0x7FFFFFFFU;
        objectIdSpin->setValue(objectId ? objectId : 1U);
        objectContentTypeCombo->setCurrentIndex(
            ObjectProgramming::contentTypeForFile(fileName) - 1);
    }
    objectWriteButton->setEnabled(true);
    objectProgress->setValue(0);
    objectProgramLog(tr("Payload: %1 байт, CRC32 0x%2")
        .arg(payload.size())
        .arg(ObjectProgramming::crc32(payload), 8, 16, QChar('0')));
}

void ex::objectProgramSend(const QByteArray &frame)
{
    if (frame.isEmpty() || frame.size() > int(sizeof(w.buf_out)))
    {
        objectProgramFail(tr("Не удалось сформировать Modbus-кадр"));
        return;
    }
    objectProgramLastRequest = frame;
    w.max_out = frame.size();
    memcpy(w.buf_out, frame.constData(), frame.size());
    w.CRC_ok = false;
    w.send_current_frame();
    sh(1);
    objectProgramTimeoutTimer->start(10000);
}

void ex::objectProgramWrite()
{
    if (objectProgramPayload.isEmpty())
    {
        objectProgramFail(tr("Сначала выберите payload"));
        return;
    }
    if (objectProgramPhase != ObjectProgramIdle)
    {
        objectProgramFail(tr("Другая операция загрузки уже выполняется"));
        return;
    }
    if (!w.com_S && (!w.port || !w.port->isOpen()))
    {
        objectProgramFail(tr("COM-порт не открыт"));
        return;
    }

    ObjectProgramming::ObjectInfo object;
    object.type = quint16(objectTypeCombo->itemData(
        objectTypeCombo->currentIndex()).toInt());
    object.flags = 0;
    if (object.type == ObjectProgramming::ObjectXipModule ||
        object.type == ObjectProgramming::ObjectLuaVm)
        object.flags |= ObjectProgramming::FlagExecutable;
    if (objectAutostartCheck->isChecked())
        object.flags |= ObjectProgramming::FlagAutostart;
    if (objectReadonlyCheck->isChecked())
        object.flags |= ObjectProgramming::FlagReadonly;
    if (objectSystemCheck->isChecked())
        object.flags |= ObjectProgramming::FlagSystem;
    if (objectCompressedCheck->isChecked())
        object.flags |= ObjectProgramming::FlagCompressed;
    object.objectId = quint32(objectIdSpin->value());
    object.generation = QDateTime::currentDateTime().toTime_t();
    if (!object.generation)
        object.generation = 1;
    object.name = objectNameEdit->text();
    object.contentType =
        object.type == ObjectProgramming::ObjectWebFile ?
        quint16(objectContentTypeCombo->currentIndex() + 1) : 0;

    const bool executable =
        object.type == ObjectProgramming::ObjectXipModule ||
        object.type == ObjectProgramming::ObjectLuaVm;
    bool addressOk = true;
    object.linkAddress = executable ?
        objectLinkAddressEdit->text().toUInt(&addressOk, 0) : 0;
    object.entryOffset = executable ?
        quint32(objectEntryOffsetSpin->value()) : 0;
    object.requiredApiVersion = executable ?
        quint16(objectApiVersionSpin->value()) : 0;
    if (!addressOk)
    {
        objectProgramFail(tr("Неверный link address"));
        return;
    }

    QString error;
    if (!ObjectProgramming::buildImage(
            objectProgramPayload, object, &objectProgramImage,
            &objectProgramImageInfo, &error))
    {
        objectProgramFail(error);
        return;
    }
    const QByteArray begin = ObjectProgramming::makeBeginConfigFrame(
        objectProgramImageInfo.imageSize,
        objectProgramImageInfo.imageCrc32, w.mb_rtu);
    if (begin.isEmpty())
    {
        objectProgramFail(tr("Не удалось сформировать начало OBJ1"));
        return;
    }

    disconnect(&w, SIGNAL(s_rd()), 0, 0);
    connect(&w, SIGNAL(s_rd()), this, SLOT(objectProgramResponse()));
    objectProgramCancelled = false;
    objectProgramOffset = 0;
    objectProgramPollCount = 0;
    objectProgramPhase = executable ?
        ObjectProgramReplaceAbortCommand : ObjectProgramBeginConfig;
    objectProgress->setRange(0, objectProgramImage.size());
    objectProgress->setValue(0);
    objectOpenButton->setEnabled(false);
    objectWriteButton->setEnabled(false);
    objectCancelButton->setEnabled(true);
    objectStatusButton->setEnabled(false);
    objectStartButton->setEnabled(false);
    objectStopButton->setEnabled(false);
    objectCatalogButton->setEnabled(false);
    objectCatalogCombo->setEnabled(false);
    objectProgramLog(tr("OBJ1 id=%1, %2 байт, %3 блоков; начало передачи...")
        .arg(object.objectId)
        .arg(objectProgramImageInfo.imageSize)
        .arg(objectProgramImageInfo.blockCount));
    if (executable)
    {
        objectProgramLog(QString::fromUtf8(
            "Отмена незавершённой операции OBJ1 перед обновлением id=%1...")
            .arg(object.objectId));
        objectProgramSend(ObjectProgramming::makeCommandFrame(
            ObjectProgramming::CommandAbort, w.mb_rtu));
    }
    else
        objectProgramSend(begin);
}

void ex::objectProgramSchedulePoll()
{
    if (++objectProgramPollCount > 300)
    {
        objectProgramFail(tr("Тайм-аут операции OBJ1"));
        return;
    }
    QTimer::singleShot(100, this, SLOT(objectProgramPoll()));
}

void ex::objectProgramPoll()
{
    if (objectProgramPhase == ObjectProgramIdle)
        return;
    objectProgramSend(
        ObjectProgramming::makeStatusReadFrame(w.mb_rtu));
}

void ex::objectProgramStartChunk()
{
    objectProgramChunk = objectProgramImage.mid(
        objectProgramOffset, ObjectProgramming::ChunkSize);
    objectProgramFrames = ObjectProgramming::makeChunkDataFrames(
        objectProgramChunk, w.mb_rtu);
    if (objectProgramFrames.isEmpty())
    {
        objectProgramFail(tr("Не удалось сформировать блок OBJ1"));
        return;
    }
    objectProgramFrameIndex = 0;
    objectProgramPhase = ObjectProgramChunkData;
    objectProgramLog(tr("Блок offset=%1, size=%2")
                     .arg(objectProgramOffset)
                     .arg(objectProgramChunk.size()));
    objectProgramSend(objectProgramFrames.first());
}

void ex::objectProgramFinish(const QString &message, bool success)
{
    objectProgramPhase = ObjectProgramIdle;
    objectProgramTimeoutTimer->stop();
    disconnect(&w, SIGNAL(s_rd()), this, SLOT(objectProgramResponse()));
    objectOpenButton->setEnabled(true);
    objectWriteButton->setEnabled(!objectProgramPayload.isEmpty());
    objectCancelButton->setEnabled(false);
    objectStatusButton->setEnabled(true);
    objectStartButton->setEnabled(true);
    objectStopButton->setEnabled(true);
    objectCatalogButton->setEnabled(true);
    objectCatalogCombo->setEnabled(true);
    if (success)
        objectProgress->setValue(objectProgress->maximum());
    objectProgramLog(message);
    if (luaComWriteActive)
    {
        luaComWriteActive = false;
        luaSetBusy(false);
        luaShowStatus(message, success);
        if (success && luaRunAfterWrite)
        {
            luaRunAfterWrite = false;
            QTimer::singleShot(0, this, SLOT(luaRun()));
        }
    }
    if (luaComReadActive)
    {
        luaComReadActive = false;
        luaSetBusy(false);
        luaShowStatus(message, success);
        if (success && luaRefreshTagsAfterRead)
            QTimer::singleShot(0, this, SLOT(luaRefreshTags()));
        else if (!success)
            luaRefreshTagsAfterRead = false;
    }
    if (luaRuntimeActive)
    {
        luaRuntimeActive = false;
        luaSetBusy(false);
        luaShowStatus(message, success);
    }
}

void ex::objectProgramRuntime(ObjectProgramPhase phase,
                              const QString &message)
{
    if (objectProgramPhase != ObjectProgramIdle)
    {
        objectProgramLog(QString::fromUtf8(
            "Операция OBJ1 уже выполняется"));
        return;
    }
    if (!w.com_S && (!w.port || !w.port->isOpen()))
    {
        objectProgramFail(QString::fromUtf8("COM-порт не открыт"));
        return;
    }
    disconnect(&w, SIGNAL(s_rd()), 0, 0);
    connect(&w, SIGNAL(s_rd()), this, SLOT(objectProgramResponse()));
    objectProgramPhase = phase;
    objectProgramPollCount = 0;
    objectOpenButton->setEnabled(false);
    objectWriteButton->setEnabled(false);
    objectCancelButton->setEnabled(false);
    objectStatusButton->setEnabled(false);
    objectStartButton->setEnabled(false);
    objectStopButton->setEnabled(false);
    objectCatalogButton->setEnabled(false);
    objectCatalogCombo->setEnabled(false);
    objectProgramLog(message);
    objectProgramSend(ObjectProgramming::makeObjectSelectFrame(
        quint32(objectIdSpin->value()), w.mb_rtu));
}

void ex::objectProgramStart()
{
    objectProgramRuntime(
        ObjectProgramStartSelect,
        QString::fromUtf8("Запуск OBJ1 id=%1...")
        .arg(objectIdSpin->value()));
}

void ex::objectProgramStop()
{
    objectProgramRuntime(
        ObjectProgramStopSelect,
        QString::fromUtf8("Остановка OBJ1 id=%1...")
        .arg(objectIdSpin->value()));
}

void ex::objectProgramStatus()
{
    objectProgramRuntime(
        ObjectProgramStatusSelect,
        QString::fromUtf8("Чтение состояния OBJ1 id=%1...")
        .arg(objectIdSpin->value()));
}

void ex::objectProgramCatalog()
{
    if (objectProgramPhase != ObjectProgramIdle)
        return;
    if (!w.com_S && (!w.port || !w.port->isOpen()))
    {
        objectProgramFail(QString::fromUtf8("COM-порт не открыт"));
        return;
    }
    disconnect(&w, SIGNAL(s_rd()), 0, 0);
    connect(&w, SIGNAL(s_rd()), this, SLOT(objectProgramResponse()));
    objectCatalogCombo->clear();
    objectCatalogObjects.clear();
    objectCatalogIndex = 0;
    objectProgramPollCount = 0;
    objectProgramPhase = ObjectProgramCatalogSelect;
    objectOpenButton->setEnabled(false);
    objectWriteButton->setEnabled(false);
    objectCancelButton->setEnabled(false);
    objectStatusButton->setEnabled(false);
    objectStartButton->setEnabled(false);
    objectStopButton->setEnabled(false);
    objectCatalogButton->setEnabled(false);
    objectCatalogCombo->setEnabled(false);
    objectProgramLog(QString::fromUtf8("Чтение каталога OBJ1..."));
    objectProgramSend(ObjectProgramming::makeObjectSelectFrame(
        quint32(objectCatalogIndex), w.mb_rtu));
}

void ex::objectCatalogSelected(int index)
{
    if (index < 0 || index >= objectCatalogObjects.size())
        return;
    const ObjectProgramming::ObjectInfo &object = objectCatalogObjects.at(index);
    objectIdSpin->setValue(int(object.objectId));
    const int typeIndex = objectTypeCombo->findData(object.type);
    if (typeIndex >= 0)
        objectTypeCombo->setCurrentIndex(typeIndex);
    objectNameEdit->setText(object.name);
    objectApiVersionSpin->setValue(object.requiredApiVersion);
    objectEntryOffsetSpin->setValue(int(object.entryOffset));
    if (object.linkAddress)
        objectLinkAddressEdit->setText(QString("0x%1").arg(
            object.linkAddress, 8, 16, QChar('0')));
    objectAutostartCheck->setChecked(
        (object.flags & ObjectProgramming::FlagAutostart) != 0);
    objectReadonlyCheck->setChecked(
        (object.flags & ObjectProgramming::FlagReadonly) != 0);
    objectSystemCheck->setChecked(
        (object.flags & ObjectProgramming::FlagSystem) != 0);
    objectCompressedCheck->setChecked(
        (object.flags & ObjectProgramming::FlagCompressed) != 0);
    if (object.type == ObjectProgramming::ObjectWebFile &&
        object.contentType >= 1 && object.contentType <= 9)
        objectContentTypeCombo->setCurrentIndex(object.contentType - 1);
}

void ex::objectProgramFail(const QString &message)
{
    objectProgramFinish(tr("Ошибка: %1").arg(message), false);
}

void ex::objectProgramCancel()
{
    if (objectProgramPhase == ObjectProgramIdle)
        return;
    objectProgramCancelled = true;
    objectProgramPhase = ObjectProgramAbortCommand;
    objectCancelButton->setEnabled(false);
    objectProgramLog(tr("Отмена OBJ1..."));
    objectProgramSend(ObjectProgramming::makeCommandFrame(
        ObjectProgramming::CommandAbort, w.mb_rtu));
}

void ex::objectProgramResponse()
{
    if (objectProgramPhase == ObjectProgramIdle)
        return;
    objectProgramTimeoutTimer->stop();
    sh(0);
    if (!w.CRC_ok)
    {
        objectProgramFail(tr("CRC ответа или ответ не получен"));
        return;
    }

    const QByteArray response(reinterpret_cast<const char *>(w.buf_in),
                              int(w.max_in));
    QString error;
    if (objectProgramPhase == ObjectProgramLuaReadData)
    {
        const int remaining = luaComReadExpected - luaComReadOffset;
        const quint16 pieceSize = quint16(qMin(remaining, 250));
        QByteArray piece;
        if (!ObjectProgramming::parsePayloadRead(
                response, pieceSize, &piece, &error))
        {
            objectProgramFail(error);
            return;
        }
        luaComReadPayload.append(piece);
        luaComReadOffset += piece.size();
        if (luaComReadOffset < luaComReadExpected)
        {
            const quint16 nextSize = quint16(qMin(
                luaComReadExpected - luaComReadOffset, 250));
            objectProgramSend(ObjectProgramming::makePayloadReadFrame(
                quint16(luaComReadOffset), nextSize, w.mb_rtu));
        }
        else
        {
            luaSourceEdit->setPlainText(QString::fromUtf8(luaComReadPayload));
            objectProgramFinish(QString::fromUtf8(
                "COM6: прочитано %1 байт").arg(luaComReadPayload.size()), true);
        }
        return;
    }
    if (objectProgramPhase == ObjectProgramCatalogHeader)
    {
        ObjectProgramming::ObjectInfo object;
        quint32 payloadSize = 0U;
        if (!ObjectProgramming::parseCatalogHeader(
                response, &object, &payloadSize, &error))
        {
            objectProgramFail(error);
            return;
        }
        if (object.objectId != objectCatalogPendingStatus.objectId)
        {
            objectProgramFail(QString::fromUtf8(
                "Каталог изменился во время чтения"));
            return;
        }
        QString purpose;
        switch (object.type)
        {
        case ObjectProgramming::ObjectXipModule: purpose = QString::fromUtf8("XIP-модуль"); break;
        case ObjectProgramming::ObjectWebFile: purpose = "Web"; break;
        case ObjectProgramming::ObjectLuaVm: purpose = "Lua VM"; break;
        case ObjectProgramming::ObjectLuaScript: purpose = QString::fromUtf8("Lua-скрипт"); break;
        case ObjectProgramming::ObjectBytecode: purpose = "Bytecode"; break;
        case ObjectProgramming::ObjectDeviceProfile: purpose = QString::fromUtf8("Профиль устройства"); break;
        case ObjectProgramming::ObjectConfiguration: purpose = QString::fromUtf8("Конфигурация"); break;
        case ObjectProgramming::ObjectTagDictionary: purpose = QString::fromUtf8("Словарь тегов"); break;
        default: purpose = QString::number(object.type); break;
        }
        const quint32 payloadAddress = 0x90000000U +
            quint32(objectCatalogPendingStatus.firstBlock) *
            ObjectProgramming::BlockSize + ObjectProgramming::HeaderSize;
        const quint16 lastBlock = quint16(
            objectCatalogPendingStatus.firstBlock +
            objectCatalogPendingStatus.blockCount - 1U);
        const QString text = QString::fromUtf8(
            "ID %1 | блоки %2…%3 (%4) | 0x%5 | %6 | %7 | %8 байт")
            .arg(object.objectId)
            .arg(objectCatalogPendingStatus.firstBlock)
            .arg(lastBlock)
            .arg(objectCatalogPendingStatus.blockCount)
            .arg(payloadAddress, 8, 16, QChar('0'))
            .arg(purpose)
            .arg(object.name)
            .arg(payloadSize);
        objectCatalogObjects.append(object);
        objectCatalogCombo->addItem(text, object.objectId);
        objectProgramLog(text);
        ++objectCatalogIndex;
        objectProgramPhase = ObjectProgramCatalogSelect;
        objectProgramSend(ObjectProgramming::makeObjectSelectFrame(
            quint32(objectCatalogIndex), w.mb_rtu));
        return;
    }
    if (objectProgramPhase == ObjectProgramBeginPoll ||
        objectProgramPhase == ObjectProgramChunkPoll ||
        objectProgramPhase == ObjectProgramCommitPoll ||
        objectProgramPhase == ObjectProgramReplaceAbortPoll ||
        objectProgramPhase == ObjectProgramReplacePoll ||
        objectProgramPhase == ObjectProgramStartPoll ||
        objectProgramPhase == ObjectProgramStopPoll ||
        objectProgramPhase == ObjectProgramStatusPoll ||
        objectProgramPhase == ObjectProgramCatalogPoll ||
        objectProgramPhase == ObjectProgramLuaReadPoll)
    {
        ObjectProgramming::Status state;
        if (!ObjectProgramming::parseStatus(response, &state, &error))
        {
            objectProgramFail(error);
            return;
        }
        if (state.status == ObjectProgramming::StatusError)
        {
            if (objectProgramPhase == ObjectProgramReplacePoll &&
                state.result == 0xFFF6U)
            {
                objectProgramPhase = ObjectProgramBeginConfig;
                objectProgramSend(ObjectProgramming::makeBeginConfigFrame(
                    objectProgramImageInfo.imageSize,
                    objectProgramImageInfo.imageCrc32, w.mb_rtu));
                return;
            }
            if (objectProgramPhase == ObjectProgramCatalogPoll &&
                state.result == 0xFFF6U)
            {
                objectProgramFinish(
                    QString::fromUtf8("Каталог прочитан: %1 объектов")
                    .arg(objectCatalogCombo->count()), true);
                return;
            }
            objectProgramFail(ObjectProgramming::resultText(state.result));
            return;
        }
        if (objectProgramPhase == ObjectProgramReplaceAbortPoll &&
            state.status == ObjectProgramming::StatusAborted)
        {
            objectProgramPhase = ObjectProgramReplaceSelect;
            objectProgramLog(QString::fromUtf8(
                "Удаление старого OBJ1 перед обновлением..."));
            objectProgramSend(ObjectProgramming::makeObjectSelectFrame(
                quint32(objectIdSpin->value()), w.mb_rtu));
            return;
        }
        if (objectProgramPhase == ObjectProgramReplacePoll &&
            state.status == ObjectProgramming::StatusComplete)
        {
            objectProgramPhase = ObjectProgramBeginConfig;
            objectProgramSend(ObjectProgramming::makeBeginConfigFrame(
                objectProgramImageInfo.imageSize,
                objectProgramImageInfo.imageCrc32, w.mb_rtu));
            return;
        }
        if (objectProgramPhase == ObjectProgramCatalogPoll &&
            state.status == ObjectProgramming::StatusPresent)
        {
            objectCatalogPendingStatus = state;
            objectProgramPhase = ObjectProgramCatalogHeader;
            objectProgramSend(ObjectProgramming::makeCatalogHeaderReadFrame(
                w.mb_rtu));
            return;
#if 0
            const quint16 type = quint16(state.crc32 >> 16);
            const quint16 flags = quint16(state.crc32);
            QString typeName;
            switch (type)
            {
            case ObjectProgramming::ObjectXipModule: typeName = "XIP"; break;
            case ObjectProgramming::ObjectWebFile: typeName = "WEB"; break;
            case ObjectProgramming::ObjectLuaVm: typeName = "Lua VM"; break;
            case ObjectProgramming::ObjectLuaScript: typeName = "Lua"; break;
            case ObjectProgramming::ObjectBytecode: typeName = "Bytecode"; break;
            case ObjectProgramming::ObjectDeviceProfile: typeName = "Profile"; break;
            case ObjectProgramming::ObjectConfiguration: typeName = "Config"; break;
            case ObjectProgramming::ObjectTagDictionary: typeName = "Tags"; break;
            default: typeName = QString::number(type); break;
            }
            const QString text = QString(
                "#%1  ID %2  %3  blocks %4+%5  gen %6%7")
                .arg(objectCatalogIndex)
                .arg(state.objectId)
                .arg(typeName)
                .arg(state.firstBlock)
                .arg(state.blockCount)
                .arg(state.written)
                .arg((flags & ObjectProgramming::FlagExecutable) ?
                     QString::fromUtf8("  исполняемый") : QString());
            objectCatalogCombo->addItem(text, state.objectId);
            objectProgramLog(text);
            ++objectCatalogIndex;
            objectProgramPhase = ObjectProgramCatalogSelect;
            objectProgramSend(ObjectProgramming::makeObjectSelectFrame(
                quint32(objectCatalogIndex), w.mb_rtu));
            return;
#endif
        }
        if (objectProgramPhase == ObjectProgramLuaReadPoll &&
            state.status == ObjectProgramming::StatusPresent)
        {
            if (!state.written || state.written > 3968U)
            {
                objectProgramFail(QString::fromUtf8(
                    "Неверный размер Lua-скрипта: %1").arg(state.written));
                return;
            }
            luaComReadExpected = int(state.written);
            luaComReadOffset = 0;
            luaComReadPayload.clear();
            const quint16 firstSize = quint16(qMin(luaComReadExpected, 250));
            objectProgramPhase = ObjectProgramLuaReadData;
            objectProgramSend(ObjectProgramming::makePayloadReadFrame(
                0U, firstSize, w.mb_rtu));
            return;
        }
        if (objectProgramPhase == ObjectProgramBeginPoll &&
            state.status == ObjectProgramming::StatusReady)
        {
            objectProgramStartChunk();
            return;
        }
        if (objectProgramPhase == ObjectProgramChunkPoll &&
            state.status == ObjectProgramming::StatusChunkOk)
        {
            const int expected =
                objectProgramOffset + objectProgramChunk.size();
            if (state.written != quint32(expected))
            {
                objectProgramFail(tr("Контроллер записал %1, ожидалось %2")
                                  .arg(state.written).arg(expected));
                return;
            }
            objectProgramOffset = expected;
            objectProgress->setValue(objectProgramOffset);
            if (objectProgramOffset < objectProgramImage.size())
            {
                objectProgramStartChunk();
            }
            else
            {
                objectProgramPhase = ObjectProgramCommitCommand;
                objectProgramSend(ObjectProgramming::makeCommandFrame(
                    ObjectProgramming::CommandCommit, w.mb_rtu));
            }
            return;
        }
        if (objectProgramPhase == ObjectProgramCommitPoll &&
            state.status == ObjectProgramming::StatusComplete)
        {
            if (state.crc32 != objectProgramImageInfo.imageCrc32)
            {
                objectProgramFail(tr("CRC32 контроллера 0x%1, файла 0x%2")
                    .arg(state.crc32, 8, 16, QChar('0'))
                    .arg(objectProgramImageInfo.imageCrc32,
                         8, 16, QChar('0')));
                return;
            }
            objectProgramFinish(
                tr("OBJ1 id=%1 записан: блок %2, блоков %3, каталог gen=%4")
                .arg(state.objectId).arg(state.firstBlock)
                .arg(state.blockCount).arg(state.directoryGeneration), true);
            return;
        }
        if (objectProgramPhase == ObjectProgramStartPoll &&
            state.status == ObjectProgramming::StatusRunning)
        {
            objectProgramFinish(
                QString::fromUtf8(
                    "OBJ1 id=%1 запущен, блок %2, блоков %3")
                .arg(state.objectId).arg(state.firstBlock)
                .arg(state.blockCount), true);
            return;
        }
        if (objectProgramPhase == ObjectProgramStopPoll &&
            state.status == ObjectProgramming::StatusStopped)
        {
            objectProgramFinish(
                QString::fromUtf8("OBJ1 id=%1 остановлен")
                .arg(state.objectId), true);
            return;
        }
        if (objectProgramPhase == ObjectProgramStatusPoll &&
            (state.status == ObjectProgramming::StatusRunning ||
             state.status == ObjectProgramming::StatusPresent ||
             state.status == ObjectProgramming::StatusStopped))
        {
            objectProgramFinish(
                QString::fromUtf8(
                    "OBJ1 id=%1: %2, блок %3, блоков %4, каталог gen=%5")
                .arg(state.objectId)
                .arg(ObjectProgramming::statusText(state.status))
                .arg(state.firstBlock).arg(state.blockCount)
                .arg(state.directoryGeneration), true);
            return;
        }
        objectProgramSchedulePoll();
        return;
    }

    if (!ObjectProgramming::isWriteAcknowledge(
            objectProgramLastRequest, response, &error))
    {
        objectProgramFail(error);
        return;
    }

    if (objectProgramPhase == ObjectProgramReplaceAbortCommand)
    {
        objectProgramPhase = ObjectProgramReplaceAbortPoll;
        objectProgramPollCount = 0;
        objectProgramSchedulePoll();
    }
    else if (objectProgramPhase == ObjectProgramReplaceSelect)
    {
        objectProgramPhase = ObjectProgramReplaceCommand;
        objectProgramSend(ObjectProgramming::makeCommandFrame(
            ObjectProgramming::CommandDelete, w.mb_rtu));
    }
    else if (objectProgramPhase == ObjectProgramReplaceCommand)
    {
        objectProgramPhase = ObjectProgramReplacePoll;
        objectProgramPollCount = 0;
        objectProgramSchedulePoll();
    }
    else if (objectProgramPhase == ObjectProgramBeginConfig)
    {
        objectProgramPhase = ObjectProgramBeginCommand;
        objectProgramSend(ObjectProgramming::makeCommandFrame(
            ObjectProgramming::CommandBegin, w.mb_rtu));
    }
    else if (objectProgramPhase == ObjectProgramBeginCommand)
    {
        objectProgramPhase = ObjectProgramBeginPoll;
        objectProgramPollCount = 0;
        objectProgramSchedulePoll();
    }
    else if (objectProgramPhase == ObjectProgramChunkData)
    {
        ++objectProgramFrameIndex;
        if (objectProgramFrameIndex < objectProgramFrames.size())
        {
            objectProgramSend(
                objectProgramFrames.at(objectProgramFrameIndex));
        }
        else
        {
            objectProgramPhase = ObjectProgramChunkMeta;
            objectProgramSend(ObjectProgramming::makeChunkMetaFrame(
                quint32(objectProgramOffset),
                quint16(objectProgramChunk.size()), w.mb_rtu));
        }
    }
    else if (objectProgramPhase == ObjectProgramChunkMeta)
    {
        objectProgramPhase = ObjectProgramChunkCommand;
        objectProgramSend(ObjectProgramming::makeCommandFrame(
            ObjectProgramming::CommandChunk, w.mb_rtu));
    }
    else if (objectProgramPhase == ObjectProgramChunkCommand)
    {
        objectProgramPhase = ObjectProgramChunkPoll;
        objectProgramPollCount = 0;
        objectProgramSchedulePoll();
    }
    else if (objectProgramPhase == ObjectProgramCommitCommand)
    {
        objectProgramPhase = ObjectProgramCommitPoll;
        objectProgramPollCount = 0;
        objectProgramSchedulePoll();
    }
    else if (objectProgramPhase == ObjectProgramAbortCommand)
    {
        objectProgramFinish(tr("Операция OBJ1 отменена"), false);
    }
    else if (objectProgramPhase == ObjectProgramStartSelect)
    {
        objectProgramPhase = ObjectProgramStartCommand;
        objectProgramSend(ObjectProgramming::makeCommandFrame(
            ObjectProgramming::CommandStart, w.mb_rtu));
    }
    else if (objectProgramPhase == ObjectProgramStopSelect)
    {
        objectProgramPhase = ObjectProgramStopCommand;
        objectProgramSend(ObjectProgramming::makeCommandFrame(
            ObjectProgramming::CommandStop, w.mb_rtu));
    }
    else if (objectProgramPhase == ObjectProgramStatusSelect)
    {
        objectProgramPhase = ObjectProgramStatusCommand;
        objectProgramSend(ObjectProgramming::makeCommandFrame(
            ObjectProgramming::CommandObjectStatus, w.mb_rtu));
    }
    else if (objectProgramPhase == ObjectProgramCatalogSelect)
    {
        objectProgramPhase = ObjectProgramCatalogCommand;
        objectProgramSend(ObjectProgramming::makeCommandFrame(
            ObjectProgramming::CommandCatalogItem, w.mb_rtu));
    }
    else if (objectProgramPhase == ObjectProgramLuaReadSelect)
    {
        objectProgramPhase = ObjectProgramLuaReadCommand;
        objectProgramSend(ObjectProgramming::makeCommandFrame(
            ObjectProgramming::CommandReadPayload, w.mb_rtu));
    }
    else if (objectProgramPhase == ObjectProgramStartCommand)
    {
        objectProgramPhase = ObjectProgramStartPoll;
        objectProgramPollCount = 0;
        objectProgramSchedulePoll();
    }
    else if (objectProgramPhase == ObjectProgramStopCommand)
    {
        objectProgramPhase = ObjectProgramStopPoll;
        objectProgramPollCount = 0;
        objectProgramSchedulePoll();
    }
    else if (objectProgramPhase == ObjectProgramStatusCommand)
    {
        objectProgramPhase = ObjectProgramStatusPoll;
        objectProgramPollCount = 0;
        objectProgramSchedulePoll();
    }
    else if (objectProgramPhase == ObjectProgramCatalogCommand)
    {
        objectProgramPhase = ObjectProgramCatalogPoll;
        objectProgramPollCount = 0;
        objectProgramSchedulePoll();
    }
    else if (objectProgramPhase == ObjectProgramLuaReadCommand)
    {
        objectProgramPhase = ObjectProgramLuaReadPoll;
        objectProgramPollCount = 0;
        objectProgramSchedulePoll();
    }
}

void ex::objectProgramTimeout()
{
    if (objectProgramPhase != ObjectProgramIdle)
        objectProgramFail(tr("Нет ответа от COM в течение 10 секунд"));
}



void ex::on_slot1RefreshButton_clicked()
{

}


void ex::on_floatConfigReadButton_clicked()
{

}

